//! Reads the shared settings file written by `touchpadflow-app` (same schema on every OS).
use serde::Deserialize;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeAction {
    Brightness,
    Volume,
    None,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SharedConfig {
    pub enabled: bool,
    pub left_edge_ratio: f32,
    pub right_edge_ratio: f32,
    pub left_action: EdgeAction,
    pub right_action: EdgeAction,
    pub left_sensitivity: f32,
    pub right_sensitivity: f32,
}

impl Default for SharedConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            left_edge_ratio: 0.22,
            right_edge_ratio: 0.22,
            left_action: EdgeAction::Brightness,
            right_action: EdgeAction::Volume,
            left_sensitivity: 1.5,
            right_sensitivity: 1.5,
        }
    }
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("touchpadflow.json")
}

pub fn modified() -> Option<SystemTime> {
    std::fs::metadata(path()).and_then(|m| m.modified()).ok()
}

pub fn load() -> SharedConfig {
    let p = path();
    let Ok(text) = std::fs::read_to_string(&p) else {
        return SharedConfig::default();
    };
    match serde_json::from_str::<SharedConfig>(&text) {
        Ok(mut c) => {
            c.left_edge_ratio = c.left_edge_ratio.clamp(0.05, 0.45);
            c.right_edge_ratio = c.right_edge_ratio.clamp(0.05, 0.45);
            c.left_sensitivity = c.left_sensitivity.clamp(0.2, 4.0);
            c.right_sensitivity = c.right_sensitivity.clamp(0.2, 4.0);
            c
        }
        Err(e) => {
            eprintln!("warning: {} is invalid ({e}); using defaults", p.display());
            SharedConfig::default()
        }
    }
}
