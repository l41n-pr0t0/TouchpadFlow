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

    /// No worker thread: values change in memory only, the system is never touched (tests).
    #[must_use]
    pub fn detached() -> Self {
        Self {
            target: Arc::new(Mutex::new(0.5)),
            cv: Arc::default(),
            running: Arc::new(AtomicBool::new(false)),
            backend: AudioBackend::Pipewire,
        }
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
    pub fn target_value(&self) -> f64 {
        *self.target.lock().unwrap()
    }

    #[must_use]
    pub fn backend(&self) -> AudioBackend {
        self.backend
    }

    fn fetch_system_volume(backend: AudioBackend) -> f64 {
        match backend {
            AudioBackend::Pipewire => {
                if let Ok(out) = Command::new("wpctl")
                    .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
                    .output()
                {
                    let s = String::from_utf8_lossy(&out.stdout);
                    let parts: Vec<&str> = s.split_whitespace().collect();
                    if parts.len() >= 2
                        && let Ok(v) = parts[1].parse::<f64>()
                    {
                        return v;
                    }
                }
            }
            AudioBackend::Pulseaudio => {
                if let Ok(out) = Command::new("pactl")
                    .args(["get-sink-volume", "@DEFAULT_SINK@"])
                    .output()
                {
                    let s = String::from_utf8_lossy(&out.stdout);
                    // Output format: "... Volume: front-left: 65536 / 100% / -0.00 dB, ..."
                    if let Some(pct_idx) = s.find('%') {
                        let prefix = &s[..pct_idx];
                        if let Some(last_space) = prefix.rfind(' ')
                            && let Ok(pct) = prefix[last_space + 1..].trim().parse::<f64>()
                        {
                            return (pct / 100.0).clamp(0.0, 1.5);
                        }
                    }
                }
            }
            AudioBackend::Alsa => {
                if let Ok(out) = Command::new("amixer").args(["sget", "Master"]).output() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    if let Some(start) = s.find('[')
                        && let Some(end) = s[start..].find("%]")
                        && let Ok(pct) = s[start + 1..start + end].parse::<f64>()
                    {
                        return (pct / 100.0).clamp(0.0, 1.5);
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
    DirectSysfs(Vec<(PathBuf, i32)>),
    Brightnessctl(Vec<String>),
    Light(String),
}

impl BrightnessBackend {
    #[must_use]
    pub fn detect() -> Option<Self> {
        let all_devs = detect_all_backlights();

        // 1. Direct sysfs write (0 subprocess overhead, works with uaccess udev rules)
        let mut writable_sysfs = Vec::new();
        for (_, dev_path, max_val) in &all_devs {
            let brightness_file = dev_path.join("brightness");
            if fs::OpenOptions::new()
                .write(true)
                .open(&brightness_file)
                .is_ok()
            {
                writable_sysfs.push((brightness_file, *max_val));
            }
        }
        if !writable_sysfs.is_empty() {
            println!(
                "Detected direct sysfs backlight control ({} device(s))",
                writable_sysfs.len()
            );
            return Some(BrightnessBackend::DirectSysfs(writable_sysfs));
        }

        // 2. brightnessctl (Fedora, Arch, Debian, Ubuntu, Alpine)
        if Command::new("brightnessctl")
            .arg("--version")
            .output()
            .is_ok()
        {
            let names: Vec<String> = all_devs.iter().map(|(n, _, _)| n.clone()).collect();
            println!("Detected brightnessctl for devices: {:?}", names);
            return Some(BrightnessBackend::Brightnessctl(names));
        }

        // 3. light (Debian, Arch, Void)
        if Command::new("light").arg("-v").output().is_ok()
            && let Some((name, _, _)) = all_devs.first()
        {
            println!("Detected 'light' utility for device: {}", name);
            return Some(BrightnessBackend::Light(name.clone()));
        }

        println!("Fallback to brightnessctl");
        Some(BrightnessBackend::Brightnessctl(
            all_devs.into_iter().map(|(n, _, _)| n).collect(),
        ))
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

    /// No worker thread: values change in memory only, the system is never touched (tests).
    #[must_use]
    pub fn detached() -> Self {
        Self {
            target: Arc::new(Mutex::new(0.5)),
            cv: Arc::default(),
            running: Arc::new(AtomicBool::new(false)),
            backend: None,
        }
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
    pub fn target_value(&self) -> f64 {
        *self.target.lock().unwrap()
    }

    #[must_use]
    pub fn backend_description(&self) -> String {
        match &self.backend {
            Some(BrightnessBackend::DirectSysfs(devices)) => {
                format!("Direct sysfs ({} devices)", devices.len())
            }
            Some(BrightnessBackend::Brightnessctl(devices)) => {
                format!("brightnessctl ({:?})", devices)
            }
            Some(BrightnessBackend::Light(d)) => format!("light (-s {})", d),
            None => "No backlight device found".into(),
        }
    }

    fn fetch_system_brightness(backend: Option<&BrightnessBackend>) -> f64 {
        match backend {
            Some(BrightnessBackend::DirectSysfs(devices)) => {
                for (path, max_val) in devices {
                    if let Ok(s) = fs::read_to_string(path)
                        && let Ok(curr) = s.trim().parse::<i32>()
                        && *max_val > 0
                    {
                        return (curr as f64 / *max_val as f64).clamp(0.01, 1.0);
                    }
                }
            }
            Some(BrightnessBackend::Brightnessctl(devices)) => {
                // First try brightnessctl default
                if let (Ok(curr), Ok(max)) = (
                    Command::new("brightnessctl").args(["get"]).output(),
                    Command::new("brightnessctl").args(["max"]).output(),
                ) {
                    let c_str = String::from_utf8_lossy(&curr.stdout);
                    let m_str = String::from_utf8_lossy(&max.stdout);
                    if let (Ok(c), Ok(m)) =
                        (c_str.trim().parse::<f64>(), m_str.trim().parse::<f64>())
                        && m > 0.0
                    {
                        return (c / m).clamp(0.01, 1.0);
                    }
                }
                // Fallback to checking specific devices
                for dev in devices {
                    if let (Ok(curr), Ok(max)) = (
                        Command::new("brightnessctl")
                            .args(["-d", dev, "get"])
                            .output(),
                        Command::new("brightnessctl")
                            .args(["-d", dev, "max"])
                            .output(),
                    ) {
                        let c_str = String::from_utf8_lossy(&curr.stdout);
                        let m_str = String::from_utf8_lossy(&max.stdout);
                        if let (Ok(c), Ok(m)) =
                            (c_str.trim().parse::<f64>(), m_str.trim().parse::<f64>())
                            && m > 0.0
                        {
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
            Some(BrightnessBackend::DirectSysfs(devices)) => {
                for (path, max_val) in devices {
                    let raw_val = ((val * *max_val as f64).round() as i32).max(1);
                    let _ = fs::write(path, raw_val.to_string());
                }
            }
            Some(BrightnessBackend::Brightnessctl(devices)) => {
                let pct_str = format!("{}%", pct_int);
                // 1. Update the system default backlight device chosen by brightnessctl
                let _ = Command::new("brightnessctl")
                    .args(["set", &pct_str, "-q"])
                    .status();
                // 2. Also update all detected physical backlight controllers (e.g. nvidia_0 and intel_backlight on hybrid laptops)
                for dev in devices {
                    let _ = Command::new("brightnessctl")
                        .args(["-d", dev, "set", &pct_str, "-q"])
                        .status();
                }
            }
            Some(BrightnessBackend::Light(dev)) => {
                let pct_str = format!("{}", pct_int);
                let _ = Command::new("light")
                    .args(["-s", dev, "-S", &pct_str])
                    .status();
            }
            None => {}
        }

        show_osd_brightness(pct_int);
    }
}

/// Discovers all laptop display panel backlight devices in /sys/class/backlight.
/// Returns Vec<(device_name, device_sysfs_path, max_brightness_value)>.
pub fn detect_all_backlights() -> Vec<(String, PathBuf, i32)> {
    let mut candidates: Vec<(String, PathBuf, i32)> = Vec::new();
    let entries = match fs::read_dir("/sys/class/backlight") {
        Ok(e) => e,
        Err(_) => return candidates,
    };

    for entry in entries.flatten() {
        if let Ok(name) = entry.file_name().into_string() {
            let path = entry.path();
            let max_val = fs::read_to_string(path.join("max_brightness"))
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok())
                .unwrap_or(100);

            candidates.push((name, path, max_val));
        }
    }

    candidates
}

fn get_qdbus_bin() -> Option<&'static str> {
    if Command::new("qdbus-qt6").arg("--version").output().is_ok() {
        Some("qdbus-qt6")
    } else if Command::new("qdbus6").arg("--version").output().is_ok() {
        Some("qdbus6")
    } else if Command::new("qdbus").arg("--version").output().is_ok() {
        Some("qdbus")
    } else {
        None
    }
}

fn show_osd_volume(pct: i32) {
    let de = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_uppercase();

    if (de.contains("KDE") || de.contains("PLASMA"))
        && let Some(qdbus) = get_qdbus_bin()
    {
        let _ = Command::new(qdbus)
            .args([
                "org.kde.plasmashell",
                "/org/kde/osdService",
                "org.kde.osdService.volumeChanged",
                &pct.to_string(),
            ])
            .status();
        return;
    }

    if Command::new("swayosd-client")
        .arg("--version")
        .output()
        .is_ok()
    {
        let _ = Command::new("swayosd-client")
            .args(["--output-volume", &pct.to_string()])
            .status();
    } else if Command::new("notify-send")
        .arg("--version")
        .output()
        .is_ok()
    {
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
    let de = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_uppercase();

    if (de.contains("KDE") || de.contains("PLASMA"))
        && let Some(qdbus) = get_qdbus_bin()
    {
        let _ = Command::new(qdbus)
            .args([
                "org.kde.plasmashell",
                "/org/kde/osdService",
                "org.kde.osdService.brightnessChanged",
                &pct.to_string(),
            ])
            .status();
        return;
    }

    if Command::new("swayosd-client")
        .arg("--version")
        .output()
        .is_ok()
    {
        let _ = Command::new("swayosd-client")
            .args(["--brightness", &pct.to_string()])
            .status();
    } else if Command::new("notify-send")
        .arg("--version")
        .output()
        .is_ok()
    {
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
    if let Some(cmd) = action
        .strip_prefix("command:")
        .or_else(|| action.strip_prefix("cmd:"))
    {
        let _ = Command::new("sh").args(["-c", cmd.trim()]).spawn();
    } else if let Some(shortcut) = action.strip_prefix("shortcut:") {
        let de = env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_uppercase();
        if de.contains("KDE") || de.contains("PLASMA") {
            let qdbus = get_qdbus_bin().unwrap_or("qdbus6");
            let _ = Command::new(qdbus)
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
            let qdbus = get_qdbus_bin().unwrap_or("qdbus6");
            let _ = Command::new(qdbus).args(&parts).spawn();
        }
    }
}
