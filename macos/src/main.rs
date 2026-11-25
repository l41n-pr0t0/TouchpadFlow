mod config;
mod controls;
mod gesture;
mod multitouch;

use std::sync::Mutex;
use clap::Parser;
use gesture::MacGestureEngine;
use multitouch::{MTContact, MultitouchDevice};

static GESTURE_ENGINE: Mutex<Option<MacGestureEngine>> = Mutex::new(None);

extern "C" fn contact_callback(
    _device: *mut std::ffi::c_void,
    contacts: *const MTContact,
    num_contacts: i32,
    _timestamp: f64,
    _frame: i32,
) -> i32 {
    if num_contacts > 0 && !contacts.is_null() {
        let slice = unsafe { std::slice::from_raw_parts(contacts, num_contacts as usize) };
        if let Ok(mut lock) = GESTURE_ENGINE.lock() {
            if let Some(ref mut engine) = *lock {
                engine.handle_contacts(slice);
            }
        }
    }
    0
}

#[derive(Parser, Debug)]
#[command(name = "touchpadflow-macos")]
#[command(
    about = "Native macOS trackpad edge gesture daemon. Settings come from touchpadflow-app \
             (~/.config/touchpadflow.json) and reload automatically."
)]
struct Cli {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Cli::parse();

    let cfg = config::load();
    println!("=== TouchpadFlow for macOS ===");
    println!("Settings: {} ({cfg:?})", config::path().display());

    {
        let mut lock = GESTURE_ENGINE.lock().unwrap();
        *lock = Some(MacGestureEngine::new(cfg));
    }

    let _dev = match MultitouchDevice::open_default(contact_callback) {
        Ok(d) => {
            println!("Connected to Apple Multitouch Trackpad.");
            d
        }
        Err(e) => {
            eprintln!("Error opening trackpad: {}", e);
            eprintln!("(Note: MultitouchSupport requires running on macOS with an Apple Trackpad)");
            return Ok(());
        }
    };

    println!("TouchpadFlow daemon active. Slide two fingers on trackpad edges.");
    println!("Press Ctrl+C to stop.");

    // Re-read the settings file whenever the app saves it.
    let mut stamp = config::modified();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let now = config::modified();
        if now != stamp {
            stamp = now;
            if let Ok(mut lock) = GESTURE_ENGINE.lock() {
                if let Some(engine) = lock.as_mut() {
                    engine.config = config::load();
                }
            }
        }
    }
}
