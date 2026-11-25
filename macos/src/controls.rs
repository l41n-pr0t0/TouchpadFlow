use std::process::Command;

pub struct MacAudio;

impl MacAudio {
    pub fn get_volume() -> f32 {
        let out = Command::new("osascript")
            .args(["-e", "output volume of (get volume settings)"])
            .output();

        if let Ok(res) = out {
            let s = String::from_utf8_lossy(&res.stdout);
            if let Ok(vol) = s.trim().parse::<f32>() {
                return (vol / 100.0).clamp(0.0, 1.0);
            }
        }
        0.5
    }

    pub fn set_volume(scalar: f32) {
        let pct = (scalar.clamp(0.0, 1.0) * 100.0).round() as i32;
        let script = format!("set volume output volume {}", pct);
        let _ = Command::new("osascript").args(["-e", &script]).status();
    }
}

pub struct MacBrightness;

impl MacBrightness {
    pub fn set_brightness(pct_delta: f32) {
        // Can call `brightness` CLI or simulated key event for macOS OSD
        let script = if pct_delta > 0.0 {
            r#"tell application "System Events" to key code 144"# // Brightness Up
        } else {
            r#"tell application "System Events" to key code 145"# // Brightness Down
        };

        let _ = Command::new("osascript").args(["-e", script]).status();
    }
}
