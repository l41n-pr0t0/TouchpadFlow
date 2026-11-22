use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioBackend {
    Pipewire,
    Pulseaudio,
    Alsa,
}

impl AudioBackend {
    #[must_use]
    pub fn detect() -> Self {
        if Command::new("wpctl").arg("--version").output().is_ok() {
            println!("Detected audio backend: PipeWire (wpctl)");
            AudioBackend::Pipewire
        } else if Command::new("pactl").arg("--version").output().is_ok() {
            println!("Detected audio backend: PulseAudio (pactl)");
            AudioBackend::Pulseaudio
        } else if Command::new("amixer").arg("--version").output().is_ok() {
            println!("Detected audio backend: ALSA (amixer)");
            AudioBackend::Alsa
        } else {
            println!("Fallback audio backend: PipeWire (default)");
            AudioBackend::Pipewire
        }
    }
}

#[derive(Debug, Clone)]
pub struct VolumeController {
    target: Arc<Mutex<f64>>,
    cv: Arc<Condvar>,
    running: Arc<AtomicBool>,
    backend: AudioBackend,
}

impl VolumeController {
    #[must_use]
    pub fn new() -> (Self, JoinHandle<()>) {
        let backend = AudioBackend::detect();
        let initial_vol = Self::fetch_system_volume(backend);
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

                    Self::apply_volume(latest, backend);
                    *t_current.lock().unwrap() = latest;
                }
            })
            .expect("failed to spawn volume thread");

        (
            Self {
                target,
                cv,
                running,
                backend,
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

    #[must_use]
    pub fn backend(&self) -> AudioBackend {
        self.backend
    }

    fn fetch_system_volume(backend: AudioBackend) -> f64 {
        match backend {
            AudioBackend::Pipewire => {
                if let Ok(out) = Command::new("wpctl").args(["get-volume", "@DEFAULT_AUDIO_SINK@"]).output() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    let parts: Vec<&str> = s.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(v) = parts[1].parse::<f64>() {
                            return v;
                        }
                    }
                }
            }
            AudioBackend::Pulseaudio => {
                if let Ok(out) = Command::new("pactl").args(["get-sink-volume", "@DEFAULT_SINK@"]).output() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    // Output format: "... Volume: front-left: 65536 / 100% / -0.00 dB, ..."
                    if let Some(pct_idx) = s.find('%') {
                        let prefix = &s[..pct_idx];
                        if let Some(last_space) = prefix.rfind(' ') {
                            if let Ok(pct) = prefix[last_space + 1..].trim().parse::<f64>() {
                                return (pct / 100.0).clamp(0.0, 1.5);
                            }
                        }
                    }
                }
            }
            AudioBackend::Alsa => {
                if let Ok(out) = Command::new("amixer").args(["sget", "Master"]).output() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    if let Some(start) = s.find('[') {
                        if let Some(end) = s[start..].find("%]") {
                            if let Ok(pct) = s[start + 1..start + end].parse::<f64>() {
                                return (pct / 100.0).clamp(0.0, 1.5);
                            }
                        }
                    }
                }
            }
        }
        0.50
    }

    fn apply_volume(val: f64, backend: AudioBackend) {
        let pct_int = (val * 100.0).round() as i32;

        match backend {
            AudioBackend::Pipewire => {
                let pct = format!("{:.3}", val);
                let _ = Command::new("wpctl")
                    .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &pct])
                    .status();
            }
            AudioBackend::Pulseaudio => {
                let pct = format!("{}%", pct_int);
                let _ = Command::new("pactl")
                    .args(["set-sink-volume", "@DEFAULT_SINK@", &pct])
                    .status();
            }
            AudioBackend::Alsa => {
                let pct = format!("{}%", pct_int);
                let _ = Command::new("amixer")
                    .args(["sset", "Master", &pct])
                    .status();
            }
        }

        show_osd_volume(pct_int);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrightnessBackend {
    DirectSysfs(PathBuf, i32),
    Brightnessctl(String),
    Light(String),
}

impl BrightnessBackend {
    #[must_use]
    pub fn detect() -> Option<Self> {
        let (dev_name, dev_path, max_val) = detect_primary_backlight_info()?;

        // 1. Direct sysfs write (0 subprocess overhead, works with udev video group or permissions)
        let brightness_file = dev_path.join("brightness");
        if fs::OpenOptions::new().write(true).open(&brightness_file).is_ok() {
            println!("Detected direct sysfs backlight control (0 subprocess overhead): {:?}", brightness_file);
            return Some(BrightnessBackend::DirectSysfs(brightness_file, max_val));
        }

        // 2. brightnessctl (Fedora, Arch, Debian, Ubuntu, Alpine)
        if Command::new("brightnessctl").arg("--version").output().is_ok() {
            println!("Detected brightnessctl for device: {}", dev_name);
            return Some(BrightnessBackend::Brightnessctl(dev_name));
        }

        // 3. light (Debian, Arch, Void)
        if Command::new("light").arg("-v").output().is_ok() {
            println!("Detected 'light' utility for device: {}", dev_name);
            return Some(BrightnessBackend::Light(dev_name));
        }

        println!("Fallback to brightnessctl: {}", dev_name);
        Some(BrightnessBackend::Brightnessctl(dev_name))
    }
}

#[derive(Debug, Clone)]
pub struct BrightnessController {
    target: Arc<Mutex<f64>>,
    cv: Arc<Condvar>,
    running: Arc<AtomicBool>,
    backend: Option<BrightnessBackend>,
}

impl BrightnessController {
    #[must_use]
    pub fn new() -> (Self, JoinHandle<()>) {
        let backend = BrightnessBackend::detect();
        let initial_bright = Self::fetch_system_brightness(backend.as_ref());
        let target = Arc::new(Mutex::new(initial_bright));
        let current = Arc::new(Mutex::new(initial_bright));
        let cv = Arc::new(Condvar::new());
        let running = Arc::new(AtomicBool::new(true));

        let t_target = Arc::clone(&target);
        let t_current = Arc::clone(&current);
        let t_cv = Arc::clone(&cv);
        let t_running = Arc::clone(&running);
        let t_backend = backend.clone();

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

                    Self::apply_brightness(latest, t_backend.as_ref());
                    *t_current.lock().unwrap() = latest;
                }
            })
            .expect("failed to spawn brightness thread");

        (
            Self {
                target,
                cv,
                running,
                backend,
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
    pub fn backend_description(&self) -> String {
        match &self.backend {
            Some(BrightnessBackend::DirectSysfs(p, max)) => format!("Direct sysfs {:?} (max: {})", p, max),
            Some(BrightnessBackend::Brightnessctl(d)) => format!("brightnessctl (-d {})", d),
            Some(BrightnessBackend::Light(d)) => format!("light (-s {})", d),
            None => "No backlight device found".into(),
        }
    }

    fn fetch_system_brightness(backend: Option<&BrightnessBackend>) -> f64 {
        match backend {
            Some(BrightnessBackend::DirectSysfs(path, max_val)) => {
                if let Ok(s) = fs::read_to_string(path) {
                    if let Ok(curr) = s.trim().parse::<i32>() {
                        if *max_val > 0 {
                            return (curr as f64 / *max_val as f64).clamp(0.01, 1.0);
                        }
                    }
                }
            }
            Some(BrightnessBackend::Brightnessctl(d)) => {
                let curr_out = Command::new("brightnessctl").args(["-d", d, "get"]).output();
                let max_out = Command::new("brightnessctl").args(["-d", d, "max"]).output();
                if let (Ok(curr), Ok(max)) = (curr_out, max_out) {
                    let c_str = String::from_utf8_lossy(&curr.stdout);
                    let m_str = String::from_utf8_lossy(&max.stdout);
                    if let (Ok(c), Ok(m)) = (c_str.trim().parse::<f64>(), m_str.trim().parse::<f64>()) {
                        if m > 0.0 {
                            return (c / m).clamp(0.01, 1.0);
                        }
                    }
                }
            }
            Some(BrightnessBackend::Light(_)) => {
                if let Ok(out) = Command::new("light").args(["-G"]).output() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    if let Ok(pct) = s.trim().parse::<f64>() {
                        return (pct / 100.0).clamp(0.01, 1.0);
                    }
                }
            }
            None => {}
        }
        0.50
    }

    fn apply_brightness(val: f64, backend: Option<&BrightnessBackend>) {
        let pct_int = (val * 100.0).round() as i32;

        match backend {
            Some(BrightnessBackend::DirectSysfs(path, max_val)) => {
                let raw_val = ((val * *max_val as f64).round() as i32).max(1);
                let _ = fs::write(path, raw_val.to_string());
            }
            Some(BrightnessBackend::Brightnessctl(d)) => {
                let pct_str = format!("{}%", pct_int);
                let _ = Command::new("brightnessctl")
                    .args(["-d", d, "set", &pct_str, "-q"])
                    .status();
            }
            Some(BrightnessBackend::Light(_)) => {
                let pct_str = format!("{}", pct_int);
                let _ = Command::new("light")
                    .args(["-S", &pct_str])
                    .status();
            }
            None => {}
        }

        show_osd_brightness(pct_int);
    }
}

/// Discovers the active laptop display panel backlight device in /sys/class/backlight.
/// Returns (device_name, device_sysfs_path, max_brightness_value).
pub fn detect_primary_backlight_info() -> Option<(String, PathBuf, i32)> {
    let entries = fs::read_dir("/sys/class/backlight").ok()?;
    let mut candidates: Vec<(String, PathBuf, bool, i32)> = Vec::new();

    for entry in entries.flatten() {
        if let Ok(name) = entry.file_name().into_string() {
            let path = entry.path();
            let is_edp = fs::read_link(&path)
                .map(|p| p.to_string_lossy().contains("eDP"))
                .unwrap_or(false);

            let max_val = fs::read_to_string(path.join("max_brightness"))
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok())
                .unwrap_or(100);

            candidates.push((name, path, is_edp, max_val));
        }
    }

    // 1. Prefer eDP internal panel
    if let Some((name, path, _, max)) = candidates.iter().find(|(_, _, is_edp, _)| *is_edp) {
        return Some((name.clone(), path.clone(), *max));
    }

    // 2. Prefer integrated graphics (intel or amd)
    if let Some((name, path, _, max)) = candidates
        .iter()
        .find(|(n, _, _, _)| n.starts_with("intel_") || n.starts_with("amdgpu_"))
    {
        return Some((name.clone(), path.clone(), *max));
    }

    // 3. Fallback to first available
    if let Some((name, path, _, max)) = candidates.first() {
        return Some((name.clone(), path.clone(), *max));
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
    } else if Command::new("notify-send").arg("--version").output().is_ok() {
        let pct_str = format!("{}%", pct);
        let _ = Command::new("notify-send")
            .args([
                "-h",
                "string:x-canonical-private-synchronous:touchpadflow",
                "-h",
                &format!("int:value:{}", pct),
                "-t",
                "700",
                "-a",
                "TouchpadFlow",
                "Volume",
                &pct_str,
            ])
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
    } else if Command::new("notify-send").arg("--version").output().is_ok() {
        let pct_str = format!("{}%", pct);
        let _ = Command::new("notify-send")
            .args([
                "-h",
                "string:x-canonical-private-synchronous:touchpadflow",
                "-h",
                &format!("int:value:{}", pct),
                "-t",
                "700",
                "-a",
                "TouchpadFlow",
                "Brightness",
                &pct_str,
            ])
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
