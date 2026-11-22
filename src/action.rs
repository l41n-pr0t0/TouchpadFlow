use std::env;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct VolumeController {
    target: Arc<Mutex<f64>>,
    cv: Arc<Condvar>,
    running: Arc<AtomicBool>,
}

impl VolumeController {
    #[must_use]
    pub fn new() -> (Self, JoinHandle<()>) {
        let initial_vol = Self::fetch_system_volume();
        let target = Arc::new(Mutex::new(initial_vol));
        let current = Arc::new(Mutex::new(initial_vol));
        let cv = Arc::new(Condvar::new());
        let running = Arc::new(AtomicBool::new(true));

        let t_target = Arc::clone(&target);
        let t_current = Arc::clone(&current);
        let t_cv = Arc::clone(&cv);
        let t_running = Arc::clone(&running);

        let handle = thread::Builder::new()
            .name("volume-worker".into())
            .spawn(move || {
                while t_running.load(Ordering::Relaxed) {
                    {
                        let mut guard = t_target.lock().unwrap();
                        while t_running.load(Ordering::Relaxed) {
                            let curr = *t_current.lock().unwrap();
                            if (*guard - curr).abs() > 0.002 {
                                break;
                            }
                            guard = t_cv.wait(guard).unwrap();
                        }
                    }
                    if !t_running.load(Ordering::Relaxed) {
                        break;
                    }

                    thread::sleep(Duration::from_millis(25));
                    let latest = *t_target.lock().unwrap();

                    Self::apply_volume(latest);
                    *t_current.lock().unwrap() = latest;
                }
            })
            .expect("failed to spawn volume thread");

        (
            Self {
                target,
                cv,
                running,
            },
            handle,
        )
    }

    pub fn adjust(&self, delta: f64) {
        let mut guard = self.target.lock().unwrap();
        let new_val = (*guard + delta).clamp(0.0, 1.50);
        *guard = new_val;
        self.cv.notify_one();
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.cv.notify_all();
    }

    fn fetch_system_volume() -> f64 {
        let output = Command::new("wpctl")
            .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
            .output();

        if let Ok(out) = output {
            let s = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(v) = parts[1].parse::<f64>() {
                    return v;
                }
            }
        }
        0.50
    }

    fn apply_volume(val: f64) {
        let pct = format!("{:.3}", val);
        let _ = Command::new("wpctl")
            .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &pct])
            .status();

        let pct_int = (val * 100.0).round() as i32;
        show_osd_volume(pct_int);
    }
}

#[derive(Debug, Clone)]
pub struct BrightnessController {
    target: Arc<Mutex<f64>>,
    cv: Arc<Condvar>,
    running: Arc<AtomicBool>,
    device_name: Option<String>,
}

impl BrightnessController {
    #[must_use]
    pub fn new() -> (Self, JoinHandle<()>) {
        let device_name = detect_primary_backlight();
        let initial_bright = Self::fetch_system_brightness(device_name.as_deref());
        let target = Arc::new(Mutex::new(initial_bright));
        let current = Arc::new(Mutex::new(initial_bright));
        let cv = Arc::new(Condvar::new());
        let running = Arc::new(AtomicBool::new(true));

        let t_target = Arc::clone(&target);
        let t_current = Arc::clone(&current);
        let t_cv = Arc::clone(&cv);
        let t_running = Arc::clone(&running);
        let t_dev = device_name.clone();

        let handle = thread::Builder::new()
            .name("brightness-worker".into())
            .spawn(move || {
                while t_running.load(Ordering::Relaxed) {
                    {
                        let mut guard = t_target.lock().unwrap();
                        while t_running.load(Ordering::Relaxed) {
                            let curr = *t_current.lock().unwrap();
                            if (*guard - curr).abs() > 0.005 {
                                break;
                            }
                            guard = t_cv.wait(guard).unwrap();
                        }
                    }
                    if !t_running.load(Ordering::Relaxed) {
                        break;
                    }

                    thread::sleep(Duration::from_millis(25));
                    let latest = *t_target.lock().unwrap();

                    Self::apply_brightness(latest, t_dev.as_deref());
                    *t_current.lock().unwrap() = latest;
                }
            })
            .expect("failed to spawn brightness thread");

        (
            Self {
                target,
                cv,
                running,
                device_name,
            },
            handle,
        )
    }

    pub fn adjust(&self, delta: f64) {
        let mut guard = self.target.lock().unwrap();
        let new_val = (*guard + delta).clamp(0.01, 1.0);
        *guard = new_val;
        self.cv.notify_one();
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.cv.notify_all();
    }

    #[must_use]
    pub fn device_name(&self) -> Option<&str> {
        self.device_name.as_deref()
    }

    fn fetch_system_brightness(dev: Option<&str>) -> f64 {
        let mut get_cmd = Command::new("brightnessctl");
        let mut max_cmd = Command::new("brightnessctl");

        if let Some(d) = dev {
            get_cmd.args(["-d", d, "get"]);
            max_cmd.args(["-d", d, "max"]);
        } else {
            get_cmd.arg("get");
            max_cmd.arg("max");
        }

        let curr_out = get_cmd.output();
        let max_out = max_cmd.output();

        if let (Ok(curr), Ok(max)) = (curr_out, max_out) {
            let c_str = String::from_utf8_lossy(&curr.stdout);
            let m_str = String::from_utf8_lossy(&max.stdout);
            if let (Ok(c), Ok(m)) = (c_str.trim().parse::<f64>(), m_str.trim().parse::<f64>()) {
                if m > 0.0 {
                    return (c / m).clamp(0.01, 1.0);
                }
            }
        }
        0.50
    }

    fn apply_brightness(val: f64, dev: Option<&str>) {
        let pct_int = (val * 100.0).round() as i32;
        let pct_str = format!("{}%", pct_int);

        let mut cmd = Command::new("brightnessctl");
        if let Some(d) = dev {
            cmd.args(["-d", d, "set", &pct_str, "-q"]);
        } else {
            cmd.args(["set", &pct_str, "-q"]);
        }
        let _ = cmd.status();

        show_osd_brightness(pct_int);
    }
}

/// Detects the primary laptop display panel backlight device in /sys/class/backlight.
/// Prioritizes devices whose sysfs symlink contains "eDP" (embedded DisplayPort).
pub fn detect_primary_backlight() -> Option<String> {
    let entries = fs::read_dir("/sys/class/backlight").ok()?;
    let mut candidates: Vec<(String, bool)> = Vec::new();

    for entry in entries.flatten() {
        if let Ok(name) = entry.file_name().into_string() {
            let is_edp = fs::read_link(entry.path())
                .map(|p| p.to_string_lossy().contains("eDP"))
                .unwrap_or(false);
            candidates.push((name, is_edp));
        }
    }

    // 1. Prefer device directly wired to eDP (laptop internal screen)
    if let Some((name, _)) = candidates.iter().find(|(_, is_edp)| *is_edp) {
        println!("Detected primary eDP display backlight: {}", name);
        return Some(name.clone());
    }

    // 2. Prefer integrated GPU backlights (intel or amd) over discrete gpu (nvidia)
    if let Some((name, _)) = candidates
        .iter()
        .find(|(n, _)| n.starts_with("intel_") || n.starts_with("amdgpu_"))
    {
        println!("Detected integrated graphics backlight: {}", name);
        return Some(name.clone());
    }

    // 3. Fallback to first available
    if let Some((name, _)) = candidates.first() {
        println!("Fallback to backlight device: {}", name);
        return Some(name.clone());
    }

    None
}

fn show_osd_volume(pct: i32) {
    let de = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();

    if de.contains("KDE") || de.contains("PLASMA") {
        let _ = Command::new("qdbus")
            .args([
                "org.kde.plasmashell",
                "/org/kde/osdService",
                "org.kde.osdService.volumeChanged",
                &pct.to_string(),
            ])
            .status();
    } else if Command::new("swayosd-client").arg("--version").output().is_ok() {
        let _ = Command::new("swayosd-client")
            .args(["--output-volume", &pct.to_string()])
            .status();
    }
}

fn show_osd_brightness(pct: i32) {
    let de = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();

    if de.contains("KDE") || de.contains("PLASMA") {
        let _ = Command::new("qdbus")
            .args([
                "org.kde.plasmashell",
                "/org/kde/osdService",
                "org.kde.osdService.brightnessChanged",
                &pct.to_string(),
            ])
            .status();
    } else if Command::new("swayosd-client").arg("--version").output().is_ok() {
        let _ = Command::new("swayosd-client")
            .args(["--brightness", &pct.to_string()])
            .status();
    }
}

pub fn execute_action(action: &str) {
    if let Some(cmd) = action.strip_prefix("command:") {
        let _ = Command::new("sh").args(["-c", cmd]).spawn();
    } else if let Some(shortcut) = action.strip_prefix("shortcut:") {
        let de = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();
        if de.contains("KDE") || de.contains("PLASMA") {
            let _ = Command::new("qdbus6")
                .args([
                    "org.kde.kglobalaccel",
                    "/component/kwin",
                    "invokeShortcut",
                    shortcut,
                ])
                .spawn();
        }
    } else if let Some(dbus) = action.strip_prefix("dbus:") {
        let parts: Vec<&str> = dbus.split_whitespace().collect();
        if !parts.is_empty() {
            let _ = Command::new("qdbus6").args(&parts).spawn();
        }
    }
}
