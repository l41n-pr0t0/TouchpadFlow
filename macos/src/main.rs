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
#[command(about = "Native macOS trackpad edge gesture daemon for volume and brightness")]
struct Cli {
    #[arg(long, default_value_t = 0.20, help = "Left edge strip width ratio (0.0 to 0.4)")]
    left_edge: f32,

    #[arg(long, default_value_t = 0.20, help = "Right edge strip width ratio (0.0 to 0.4)")]
    right_edge: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    println!("=== TouchpadFlow for macOS ===");
    println!("Left Edge (Brightness): <= {:.0}%", args.left_edge * 100.0);
    println!("Right Edge (Volume):    >= {:.0}%", (1.0 - args.right_edge) * 100.0);

    {
        let mut lock = GESTURE_ENGINE.lock().unwrap();
        *lock = Some(MacGestureEngine::new(args.left_edge, args.right_edge));
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

    // Keep active
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
