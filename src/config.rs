use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_left_edge")]
    pub left_edge_ratio: f32,

    #[serde(default = "default_right_edge")]
    pub right_edge_ratio: f32,

    #[serde(default = "default_edge_threshold")]
    pub edge_trigger_threshold: f64,

    #[serde(default = "default_settle_window")]
    pub settle_window_ms: u64,

    #[serde(default = "default_volume_step")]
    pub volume_step: f64,

    #[serde(default = "default_brightness_step")]
    pub brightness_step: f64,

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
}

fn default_left_edge() -> f32 {
    0.22
}

fn default_right_edge() -> f32 {
    0.22
}

fn default_edge_threshold() -> f64 {
    35.0
}

fn default_settle_window() -> u64 {
    25
}

fn default_volume_step() -> f64 {
    0.02
}

fn default_brightness_step() -> f64 {
    0.02
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
    let mut map = HashMap::new();
    map.insert("three_finger_tap".into(), "key:BTN_MIDDLE".into());
    map.insert("swipe3_left".into(), "key:KEY_PREVIOUSSONG".into());
    map.insert("swipe3_right".into(), "key:KEY_NEXTSONG".into());
    map.insert("pinch3_in".into(), "key:KEY_PLAYPAUSE".into());
    map.insert("pinch3_out".into(), "key:KEY_PLAYPAUSE".into());
    map.insert("swipe4_up".into(), "shortcut:Overview".into());
    map.insert("swipe4_down".into(), "shortcut:Show Desktop".into());
    map.insert("swipe4_left".into(), "shortcut:Switch to Previous Desktop".into());
    map.insert("swipe4_right".into(), "shortcut:Switch to Next Desktop".into());
    map
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            left_edge_ratio: default_left_edge(),
            right_edge_ratio: default_right_edge(),
            edge_trigger_threshold: default_edge_threshold(),
            settle_window_ms: default_settle_window(),
            volume_step: default_volume_step(),
            brightness_step: default_brightness_step(),
            tap_threshold: default_tap_threshold(),
            tap_duration_limit_s: default_tap_duration(),
            four_finger_tap_duration_limit_s: default_four_finger_tap_duration(),
            discrete_threshold: default_discrete_threshold(),
            shortcuts: default_shortcuts(),
        }
    }
}

impl AppConfig {
    pub fn load_or_default(path_opt: Option<&Path>) -> Self {
        let Some(path) = path_opt else {
            return Self::default();
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                eprintln!("warning: could not parse config at {:?}: {}", path, e);
                Self::default()
            }),
            Err(e) => {
                eprintln!("warning: failed reading config file {:?}: {}", path, e);
                Self::default()
            }
        }
    }
}
