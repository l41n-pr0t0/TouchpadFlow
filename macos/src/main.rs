mod config;
mod controls;
mod gesture;
mod multitouch;

use gesture::MacGestureEngine;
use multitouch::{MTContact, MultitouchDevice};
use std::sync::Mutex;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopDefaultMode: *const std::ffi::c_void;
    fn CFRunLoopRunInMode(mode: *const std::ffi::c_void, seconds: f64, handled: u8) -> i32;
}

static GESTURE_ENGINE: Mutex<Option<MacGestureEngine>> = Mutex::new(None);

extern "C" fn contact_callback(
    _device: *mut std::ffi::c_void,
    contacts: *const MTContact,
    num_contacts: i32,
    _timestamp: f64,
    _frame: i32,
) -> i32 {
    // 0 contacts (all fingers lifted) must still reach the engine so it can reset.
    let slice: &[MTContact] = if num_contacts > 0 && !contacts.is_null() {
        unsafe { std::slice::from_raw_parts(contacts, num_contacts as usize) }
    } else {
        &[]
    };
    if let Ok(mut lock) = GESTURE_ENGINE.lock() {
        if let Some(ref mut engine) = *lock {
            engine.handle_contacts(slice);
        }
    }
    0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        // Multitouch frames are delivered through this thread's run loop, so it must run one.
        unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 1.0, 0) };
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
