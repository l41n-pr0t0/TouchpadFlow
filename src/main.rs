mod action;
mod config;
mod device;
mod error;
mod gesture;
mod gui;
mod gui_assets;

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use evdev::Device;

use action::{BrightnessController, VolumeController};
use config::AppConfig;
use device::{find_touchpad, ActiveDevice, GrabGuard, TouchpadMetrics, VirtualOutput};
use error::{AppError, Result};
use gesture::GestureEngine;

#[derive(Parser, Debug)]
#[command(name = "touchpadflow")]
#[command(about = "Universal Linux touchpad gesture daemon with two-finger edge volume and brightness controls")]
struct Cli {
    #[arg(short, long, help = "Path to the touchpad evdev device (e.g. /dev/input/event9)")]
    device: Option<PathBuf>,

    #[arg(short, long, help = "Path to JSON configuration file")]
    config: Option<PathBuf>,

    #[arg(long, default_value_t = true, help = "Exclusively grab the physical touchpad device")]
    grab: bool,

    #[arg(long, help = "Monitor events without grabbing or filtering")]
    monitor: bool,

    #[arg(long, help = "Run in background as a daemon (portable across non-systemd distros like Alpine/Void/Gentoo)")]
    daemon: bool,

    #[arg(long, help = "Launch the interactive TouchpadFlow Control Center GUI")]
    gui: bool,

    #[arg(long, default_value_t = 8123, help = "Port for the Control Center GUI")]
    port: u16,

    #[arg(long, help = "Emergency toggle: pauses or resumes the running daemon")]
    toggle: bool,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    if args.gui {
        return gui::start_gui_server(args.port);
    }

    if args.toggle {
        // Find running touchpadflow and send SIGUSR1 (emergency toggle)
        let _ = Command::new("pkill").args(["-SIGUSR1", "-f", "touchpadflow"]).status();
        println!("Sent emergency toggle signal (SIGUSR1) to TouchpadFlow daemon.");
        return Ok(());
    }

    if args.daemon {
        let current_exe = std::env::current_exe().map_err(AppError::Io)?;
        let mut child_cmd = Command::new(current_exe);
        if let Some(ref d) = args.device {
            child_cmd.arg("-d").arg(d);
        }
        if let Some(ref c) = args.config {
            child_cmd.arg("-c").arg(c);
        }
        if !args.grab {
            child_cmd.arg("--grab=false");
        }
        let child = child_cmd.spawn().map_err(AppError::Io)?;
        println!("TouchpadFlow daemon spawned in background with PID: {}", child.id());
        return Ok(());
    }

    // Set panic hook to ensure crash reports never leave hardware in an unusable state
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("\n[EMERGENCY] TouchpadFlow encountered an unexpected panic: {}", info);
        eprintln!("[EMERGENCY] Releasing physical device grab and restoring native touchpad state...");
        default_panic(info);
    }));

    let grab_mode = args.grab && !args.monitor;
    let cfg = AppConfig::load_or_default(args.config.as_deref());

    // Signal flags
    let running = Arc::new(AtomicBool::new(true));
    let paused = Arc::new(AtomicBool::new(false));

    let r = Arc::clone(&running);
    let p = Arc::clone(&paused);

    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&r))
        .map_err(AppError::Io)?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&r))
        .map_err(AppError::Io)?;
    signal_hook::flag::register(signal_hook::consts::SIGUSR1, Arc::clone(&p))
        .map_err(AppError::Io)?;

    // Worker threads
    let (volume, vol_worker) = VolumeController::new();
    let (brightness, bright_worker) = BrightnessController::new();

    println!("========================================================");
    println!(" TouchpadFlow v0.1.0 — Least Privilege & Fault Tolerant");
    println!(" Audio: {:?} | Backlight: {}", volume.backend(), brightness.backend_description());
    println!(" Emergency toggle: run 'touchpadflow --toggle' or send SIGUSR1");
    println!("========================================================");

    // Reconnection outer loop for fault tolerance
    while running.load(Ordering::Relaxed) {
        let dev_path = match &args.device {
            Some(p) => p.clone(),
            None => match find_touchpad() {
                Some(p) => p,
                None => {
                    eprintln!("No touchpad found. Retrying in 2 seconds...");
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
            },
        };

        println!("Connecting to touchpad at {:?}", dev_path);
        let dev = match Device::open(&dev_path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed opening {:?}: {}. Retrying in 2 seconds...", dev_path, e);
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        let metrics = TouchpadMetrics::from_device(&dev);
        println!(
            "Touchpad calibrated: bounds X=[{}..{}], Y=[{}..{}]",
            metrics.min_x, metrics.max_x, metrics.min_y, metrics.max_y
        );

        let output = match VirtualOutput::new(&dev, grab_mode) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("UInput initialization error: {}. Retrying...", e);
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        let mut active_device = if grab_mode {
            match GrabGuard::new(dev) {
                Ok(g) => {
                    println!("Exclusively grabbed device at {:?}", dev_path);
                    ActiveDevice::Grabbed(g)
                }
                Err(e) => {
                    eprintln!("Could not grab device: {}. Running in monitor mode.", e);
                    return Err(AppError::Io(e));
                }
            }
        } else {
            ActiveDevice::Ungrabbed(dev)
        };

        let mut engine = GestureEngine::new(
            cfg.clone(),
            metrics,
            volume.clone(),
            brightness.clone(),
            output,
        );

        // Active event loop
        while running.load(Ordering::Relaxed) {
            // Check emergency pause state
            if p.load(Ordering::Relaxed) {
                active_device.pause();
                let _ = engine.emergency_reset();
                std::thread::sleep(Duration::from_millis(100));
                continue;
            } else {
                active_device.resume();
            }

            // Watchdog check to prevent stuck contacts
            let _ = engine.check_watchdog();

            match active_device.device_mut().fetch_events() {
                Ok(events) => {
                    for ev in events {
                        if let Err(e) = engine.handle_event(ev) {
                            eprintln!("Frame processing error: {e}");
                        }
                    }
                }
                Err(e) => {
                    // Check if device was physically disconnected
                    if e.kind() == std::io::ErrorKind::NotFound || e.raw_os_error() == Some(19) {
                        eprintln!("[FAULT] Touchpad disconnected ({:?}). Cleaning up...", dev_path);
                        let _ = engine.emergency_reset();
                        break;
                    }
                    if running.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        }

        active_device.pause();
        let _ = engine.emergency_reset();

        if !running.load(Ordering::Relaxed) {
            break;
        }
    }

    println!("Cleaning up and exiting TouchpadFlow...");
    volume.stop();
    brightness.stop();
    let _ = vol_worker.join();
    let _ = bright_worker.join();
    println!("Graceful shutdown complete.");

    Ok(())
}
