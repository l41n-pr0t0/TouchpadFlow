use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// What a two-finger vertical slide on an edge zone controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeAction {
    Brightness,
    Volume,
    None,
}

/// UI theme preference for the settings app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
    /// Washi paper, sumi ink and a vermilion seal accent.
    Sumi,
}

/// A custom gesture-to-action macro rule, optionally scoped to specific apps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroRule {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub name: String,
    /// Case-insensitive substrings matched against the active window (empty = all apps).
    #[serde(default)]
    pub apps: Vec<String>,
    /// Gesture name, e.g. "swipe3_right", "three_finger_tap", "swipe4_up".
    pub gesture: String,
    /// Action: key combo/sequence ("ctrl+tab", "ctrl+l, enter"), "command:...", "key:...", or "shortcut:...".
    pub action: String,
}

pub const GESTURE_NAMES: &[(&str, &str)] = &[
    ("three_finger_tap", "Three-finger tap"),
    ("four_finger_tap", "Four-finger tap"),
    ("swipe3_left", "Three-finger swipe left"),
    ("swipe3_right", "Three-finger swipe right"),
    ("swipe3_up", "Three-finger swipe up"),
    ("swipe3_down", "Three-finger swipe down"),
    ("swipe4_left", "Four-finger swipe left"),
    ("swipe4_right", "Four-finger swipe right"),
    ("swipe4_up", "Four-finger swipe up"),
    ("swipe4_down", "Four-finger swipe down"),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    /// Master switch. false = the engine passes the touchpad through untouched.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Width of the left/right edge zones as a fraction of the touchpad width.
    #[serde(default = "default_edge")]
    pub left_edge_ratio: f32,
    #[serde(default = "default_edge")]
    pub right_edge_ratio: f32,

    #[serde(default = "default_left_action")]
    pub left_action: EdgeAction,
    #[serde(default = "default_right_action")]
    pub right_action: EdgeAction,

    /// Fraction of the full 0-100% range changed by one full-height slide.
    #[serde(default = "default_sensitivity")]
    pub left_sensitivity: f64,
    #[serde(default = "default_sensitivity")]
    pub right_sensitivity: f64,

    /// Vertical dead zone before an edge slide starts changing the value (raw device units).
    #[serde(default = "default_edge_threshold")]
    pub edge_trigger_threshold: f64,

    #[serde(default = "default_settle_window")]
    pub settle_window_ms: u64,

    #[serde(default = "default_tap_threshold")]
    pub tap_threshold: f64,

    #[serde(default = "default_tap_duration")]
    pub tap_duration_limit_s: f64,

    #[serde(default = "default_four_finger_tap_duration")]
    pub four_finger_tap_duration_limit_s: f64,

    #[serde(default = "default_discrete_threshold")]
    pub discrete_threshold: f64,

    #[serde(default = "default_shortcuts")]
    pub shortcuts: HashMap<String, String>,

    /// UI theme preference for the settings app.
    #[serde(default)]
    pub theme: ThemeMode,

    /// Draw the ink page (drips, halftone, hatching) behind the settings app.
    #[serde(default = "default_true")]
    pub ink_page: bool,

    /// Custom gesture macros (optionally scoped to specific apps).
    #[serde(default)]
    pub macros: Vec<MacroRule>,
}

pub const EDGE_RATIO_RANGE: (f32, f32) = (0.05, 0.45);
pub const SENSITIVITY_RANGE: (f64, f64) = (0.2, 4.0);

fn default_true() -> bool {
    true
}
fn default_edge() -> f32 {
    0.22
}
fn default_left_action() -> EdgeAction {
    EdgeAction::Brightness
}
fn default_right_action() -> EdgeAction {
    EdgeAction::Volume
}
fn default_sensitivity() -> f64 {
    1.5
}
fn default_edge_threshold() -> f64 {
    35.0
}
fn default_settle_window() -> u64 {
    25
}
fn default_tap_threshold() -> f64 {
    150.0
}
fn default_tap_duration() -> f64 {
    0.30
}
fn default_four_finger_tap_duration() -> f64 {
    0.40
}
fn default_discrete_threshold() -> f64 {
    120.0
}

fn default_shortcuts() -> HashMap<String, String> {
    [
        ("three_finger_tap", "key:BTN_MIDDLE"),
        ("swipe3_left", "key:KEY_PREVIOUSSONG"),
        ("swipe3_right", "key:KEY_NEXTSONG"),
        ("swipe4_up", "shortcut:Overview"),
        ("swipe4_down", "shortcut:Show Desktop"),
        ("swipe4_left", "shortcut:Switch to Previous Desktop"),
        ("swipe4_right", "shortcut:Switch to Next Desktop"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

impl Default for AppConfig {
    fn default() -> Self {
        // Empty JSON object = every serde default, so the two never drift apart.
        serde_json::from_str("{}").expect("defaults deserialize")
    }
}

impl AppConfig {
    /// Clamp hand-edited or out-of-range values to something the engine can run with.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        let (lo, hi) = EDGE_RATIO_RANGE;
        self.left_edge_ratio = finite_or(self.left_edge_ratio, default_edge()).clamp(lo, hi);
        self.right_edge_ratio = finite_or(self.right_edge_ratio, default_edge()).clamp(lo, hi);
        let (lo, hi) = SENSITIVITY_RANGE;
        self.left_sensitivity =
            finite_or(self.left_sensitivity, default_sensitivity()).clamp(lo, hi);
        self.right_sensitivity =
            finite_or(self.right_sensitivity, default_sensitivity()).clamp(lo, hi);
        self.edge_trigger_threshold =
            finite_or(self.edge_trigger_threshold, default_edge_threshold()).clamp(0.0, 500.0);
        self.settle_window_ms = self.settle_window_ms.clamp(0, 200);
        for m in &mut self.macros {
            m.apps = m
                .apps
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            m.gesture = m.gesture.trim().to_string();
            m.action = m.action.trim().to_string();
            m.name = m.name.trim().to_string();
        }
        self
    }

    /// Strict load: Err carries a human-readable reason (missing file is not an error).
    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str::<Self>(&content)
                .map(Self::sanitized)
                .map_err(|e| format!("{} is not valid TouchpadFlow config: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("cannot read {}: {e}", path.display())),
        }
    }

    /// Lenient load for the daemon: falls back to defaults and logs why.
    pub fn load_or_default(path_opt: Option<&Path>) -> Self {
        let path = path_opt.map_or_else(config_path, Path::to_path_buf);
        Self::load(&path).unwrap_or_else(|e| {
            eprintln!("warning: {e}; using defaults");
            Self::default()
        })
    }

    /// Atomic write (temp file + rename) so a crash never leaves a half-written config.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)
    }
}

fn finite_or<T: Into<f64> + Copy>(v: T, fallback: T) -> T {
    if v.into().is_finite() { v } else { fallback }
}

/// Shared config location read by every engine (Linux, macOS, Windows) and the app.
#[must_use]
pub fn config_path() -> PathBuf {
    if cfg!(windows) {
        let base = std::env::var_os("APPDATA").map_or_else(|| PathBuf::from("."), PathBuf::from);
        return base.join("TouchpadFlow").join("touchpadflow.json");
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("touchpadflow.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_left_brightness_right_volume() {
        let c = AppConfig::default();
        assert!(c.enabled);
        assert_eq!(c.left_action, EdgeAction::Brightness);
        assert_eq!(c.right_action, EdgeAction::Volume);
        assert_eq!(c.theme, ThemeMode::System);
        assert!(c.ink_page);
    }

    #[test]
    fn old_config_files_still_load_and_bad_values_are_clamped() {
        let c: AppConfig =
            serde_json::from_str(r#"{"left_edge_ratio": 0.9, "volume_step": 0.02}"#).unwrap();
        let c = c.sanitized();
        assert_eq!(c.left_edge_ratio, EDGE_RATIO_RANGE.1);
        assert_eq!(c.right_action, EdgeAction::Volume);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("tpf-test-{}", std::process::id()));
        let path = dir.join("touchpadflow.json");
        let c = AppConfig {
            enabled: false,
            left_action: EdgeAction::Volume,
            ..AppConfig::default()
        };
        c.save(&path).unwrap();
        assert_eq!(AppConfig::load(&path).unwrap(), c);
        fs::write(&path, "{not json").unwrap();
        assert!(AppConfig::load(&path).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
