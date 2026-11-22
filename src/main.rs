mod action;
mod config;
mod device;
mod error;
mod gesture;

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use evdev::Device;

use action::{BrightnessController, VolumeController};
use config::AppConfig;
use device::{find_touchpad, TouchpadMetrics, VirtualOutput};
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
}

fn main() -> Result<()> {
    let args = Cli::parse();

    if args.daemon {
        // Fork/spawn background child process if --daemon passed
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

    let grab_mode = args.grab && !args.monitor;
    let cfg = AppConfig::load_or_default(args.config.as_deref());

    let dev_path = match args.device {
        Some(p) => p,
        None => find_touchpad().ok_or(AppError::DeviceNotFound)?,
    };

    println!("Target touchpad: {:?}", dev_path);
    let mut dev = Device::open(&dev_path).map_err(|e| AppError::DeviceOpen {
        path: dev_path.display().to_string(),
        source: e,
    })?;

    let metrics = TouchpadMetrics::from_device(&dev);
    println!(
        "Touchpad bounds: X=[{}..{}], Y=[{}..{}]",
        metrics.min_x, metrics.max_x, metrics.min_y, metrics.max_y
    );
    println!(
        "Zones: Left Edge (Brightness) <= {:.0}%, Right Edge (Volume) >= {:.0}%",
        cfg.left_edge_ratio * 100.0,
        (1.0 - cfg.right_edge_ratio) * 100.0
    );

    let (volume, vol_worker) = VolumeController::new();
    let (brightness, bright_worker) = BrightnessController::new();

    println!("Audio backend: {:?}", volume.backend());
    println!("Brightness backend: {}", brightness.backend_description());

    let output = VirtualOutput::new(&dev, grab_mode)?;

    if grab_mode {
        dev.grab().map_err(AppError::Io)?;
        println!("Exclusively grabbed device at {:?}", dev_path);
    } else {
        println!("Running in passive monitor mode (no device grab)");
    }

    let mut engine = GestureEngine::new(cfg, metrics, volume.clone(), brightness.clone(), output);

    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);

    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&r))
        .map_err(AppError::Io)?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&r))
        .map_err(AppError::Io)?;

    println!("TouchpadFlow daemon active. Press Ctrl+C to terminate.");

    while running.load(Ordering::Relaxed) {
        match dev.fetch_events() {
            Ok(events) => {
                for ev in events {
                    if let Err(e) = engine.handle_event(ev) {
                        eprintln!("Error handling frame: {e}");
                    }
                }
            }
            Err(e) => {
                if running.load(Ordering::Relaxed) {
                    eprintln!("Event fetch error: {e}");
                }
                break;
            }
        }
    }

    println!("Shutting down TouchpadFlow...");
    if grab_mode {
        let _ = dev.ungrab();
    }
    volume.stop();
    brightness.stop();
    let _ = vol_worker.join();
    let _ = bright_worker.join();
    println!("Shutdown clean.");

    Ok(())
}
