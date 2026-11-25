use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use clap::Parser;
use evdev::Device;

use touchpadflow::action::{BrightnessController, VolumeController};
use touchpadflow::config::{self, AppConfig};
use touchpadflow::device::{
    ActiveDevice, GrabGuard, TouchpadMetrics, VirtualOutput, find_touchpad,
};
use touchpadflow::error::{AppError, Result};
use touchpadflow::gesture::GestureEngine;
use touchpadflow::ipc::{self, IpcServer};

#[derive(Parser, Debug)]
#[command(name = "touchpadflow")]
#[command(
    about = "Universal Linux touchpad gesture daemon with two-finger edge volume and brightness controls"
)]
struct Cli {
    #[arg(
        short,
        long,
        help = "Path to the touchpad evdev device (e.g. /dev/input/event9)"
    )]
    device: Option<PathBuf>,

    #[arg(short, long, help = "Path to JSON configuration file")]
    config: Option<PathBuf>,

    #[arg(
        long,
        default_value_t = true,
        help = "Exclusively grab the physical touchpad device"
    )]
    grab: bool,

    #[arg(long, help = "Monitor events without grabbing or filtering")]
    monitor: bool,

    #[arg(
        long,
        help = "Run in background as a daemon (portable across non-systemd distros like Alpine/Void/Gentoo)"
    )]
    daemon: bool,

    #[arg(long, help = "Emergency toggle: pauses or resumes the running daemon")]
    toggle: bool,

    #[arg(long, help = "Pause the running daemon (releases hardware grab)")]
    pause: bool,

    #[arg(long, help = "Resume the running daemon")]
    resume: bool,

    #[arg(long, help = "Query live daemon status")]
    status: bool,

    #[arg(long, help = "Reload configuration from disk in running daemon")]
    reload: bool,
}

pub fn main() -> Result<()> {
    let args = Cli::parse();

    if args.toggle {
        match ipc::send_command("toggle") {
            Ok(resp) => {
                println!("{resp}");
                return Ok(());
            }
            Err(_) => {
                let _ = Command::new("pkill")
                    .args(["-SIGUSR1", "-x", "touchpadflow"])
                    .status();
                println!("Sent SIGUSR1 toggle signal to TouchpadFlow daemon.");
                return Ok(());
            }
        }
    }

    if args.pause {
        match ipc::send_command("pause") {
            Ok(resp) => {
                println!("{resp}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error connecting to TouchpadFlow daemon: {e}");
                return Ok(());
            }
        }
    }

    if args.resume {
        match ipc::send_command("resume") {
            Ok(resp) => {
                println!("{resp}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error connecting to TouchpadFlow daemon: {e}");
                return Ok(());
            }
        }
    }

    if args.status {
        match ipc::send_command("status") {
            Ok(resp) => {
                println!("{resp}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("TouchpadFlow daemon is not running ({e})");
                return Ok(());
            }
        }
    }

    if args.reload {
        match ipc::send_command("reload") {
            Ok(resp) => {
                println!("{resp}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error connecting to TouchpadFlow daemon: {e}");
                return Ok(());
            }
        }
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
        println!(
            "TouchpadFlow daemon spawned in background with PID: {}",
            child.id()
        );
        return Ok(());
    }

    // Set panic hook to ensure crash reports never leave hardware in an unusable state
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!(
            "\n[EMERGENCY] TouchpadFlow encountered an unexpected panic: {}",
            info
        );
        eprintln!(
            "[EMERGENCY] Releasing physical device grab and restoring native touchpad state..."
        );
        default_panic(info);
    }));

    let grab_mode = args.grab && !args.monitor;
    let cfg_path = args.config.clone().unwrap_or_else(config::config_path);
    let mut cfg = AppConfig::load_or_default(Some(&cfg_path));

    // Signal and state flags
    let running = Arc::new(AtomicBool::new(true));
    let paused = Arc::new(AtomicBool::new(false));
    let reload_flag = Arc::new(AtomicBool::new(false));
    let device_desc = Arc::new(Mutex::new("Initializing".to_string()));

    // Dedicated signal handling thread with bidirectional SIGUSR1 toggle
    let mut signals = signal_hook::iterator::Signals::new([
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGUSR1,
    ])
    .map_err(AppError::Io)?;

    let sig_running = Arc::clone(&running);
    let sig_paused = Arc::clone(&paused);
    thread::spawn(move || {
        for sig in signals.forever() {
            match sig {
                signal_hook::consts::SIGINT | signal_hook::consts::SIGTERM => {
                    sig_running.store(false, Ordering::SeqCst);
                    // The event read blocks until the next touch; if it does not return soon,
                    // exit anyway. Closing the fds releases the grab and destroys the uinput pads.
                    thread::sleep(Duration::from_millis(500));
                    std::process::exit(0);
                }
                signal_hook::consts::SIGUSR1 => {
                    let prev = sig_paused.fetch_xor(true, Ordering::SeqCst);
                    eprintln!(
                        "[SIGNAL] SIGUSR1: Toggled daemon state to {}",
                        if !prev { "paused" } else { "active" }
                    );
                }
                _ => {}
            }
        }
    });

    // Start Unix domain socket IPC server
    let _ipc_server = IpcServer::start(
        Arc::clone(&running),
        Arc::clone(&paused),
        Arc::clone(&reload_flag),
        Arc::clone(&device_desc),
    )
    .map_err(AppError::Io)?;

    // Worker threads
    let (volume, vol_worker) = VolumeController::new();
    let (brightness, bright_worker) = BrightnessController::new();

    println!("========================================================");
    println!(" TouchpadFlow v{}", env!("CARGO_PKG_VERSION"));
    println!(
        " Audio: {:?} | Backlight: {}",
        volume.backend(),
        brightness.backend_description()
    );
    println!(" Control IPC socket: {:?}", ipc::socket_path());
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
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
            },
        };

        println!("Connecting to touchpad at {:?}", dev_path);
        let dev = match Device::open(&dev_path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!(
                    "Failed opening {:?}: {}. Retrying in 2 seconds...",
                    dev_path, e
                );
                thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        let metrics = TouchpadMetrics::from_device(&dev);
        println!(
            "Touchpad calibrated: bounds X=[{}..{}], Y=[{}..{}]",
            metrics.min_x, metrics.max_x, metrics.min_y, metrics.max_y
        );

        if let Ok(mut lock) = device_desc.lock() {
            *lock = dev_path.display().to_string();
        }

        let output = match VirtualOutput::new(&dev, grab_mode) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("UInput initialization error: {}. Retrying...", e);
                thread::sleep(Duration::from_secs(2));
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
            // Live configuration reload check
            if reload_flag.load(Ordering::Relaxed) {
                reload_flag.store(false, Ordering::Relaxed);
                // A broken file keeps the running config instead of silently resetting it.
                match AppConfig::load(&cfg_path) {
                    Ok(new_cfg) => {
                        println!("Live config reloaded from {}", cfg_path.display());
                        cfg = new_cfg.clone();
                        engine.update_config(new_cfg);
                    }
                    Err(e) => eprintln!("Config reload rejected: {e}"),
                }
            }

            // Paused via IPC/SIGUSR1, or switched off in the app: hand the pad back to the OS.
            if paused.load(Ordering::Relaxed) || !engine.enabled() {
                active_device.pause();
                let _ = engine.emergency_reset();
                thread::sleep(Duration::from_millis(50));
                continue;
            } else {
                active_device.resume();
            }

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
                        eprintln!(
                            "[FAULT] Touchpad disconnected ({:?}). Cleaning up...",
                            dev_path
                        );
                        let _ = engine.emergency_reset();
                        break;
                    }
                    if running.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(10));
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
