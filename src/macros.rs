//! Keyboard macro parsing, execution helpers, and active-window matching.

use std::process::Command;

#[cfg(target_os = "linux")]
use evdev::KeyCode;

/// Platform-independent key identifier for macro combos and sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MacroKey {
    LeftCtrl,
    RightCtrl,
    LeftShift,
    RightShift,
    LeftAlt,
    RightAlt,
    LeftMeta,
    RightMeta,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Tab,
    Enter,
    Esc,
    Space,
    Backspace,
    Delete,
    Insert,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Minus,
    Equal,
    LeftBrace,
    RightBrace,
    Semicolon,
    Apostrophe,
    Grave,
    Backslash,
    Comma,
    Dot,
    Slash,
    CapsLock,
    Print,
    BtnLeft,
    BtnRight,
    BtnMiddle,
    PlayPause,
    NextSong,
    PrevSong,
    VolumeUp,
    VolumeDown,
    Mute,
    BrightnessUp,
    BrightnessDown,
}

impl MacroKey {
    pub fn is_modifier(self) -> bool {
        matches!(
            self,
            Self::LeftCtrl
                | Self::RightCtrl
                | Self::LeftShift
                | Self::RightShift
                | Self::LeftAlt
                | Self::RightAlt
                | Self::LeftMeta
                | Self::RightMeta
        )
    }

    pub fn parse(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "lctrl" | "leftctrl" | "key_leftctrl" => Some(Self::LeftCtrl),
            "rctrl" | "rightctrl" | "key_rightctrl" => Some(Self::RightCtrl),
            "shift" | "lshift" | "leftshift" | "key_leftshift" => Some(Self::LeftShift),
            "rshift" | "rightshift" | "key_rightshift" => Some(Self::RightShift),
            "alt" | "lalt" | "leftalt" | "opt" | "option" | "key_leftalt" => Some(Self::LeftAlt),
            "ralt" | "rightalt" | "altgr" | "key_rightalt" => Some(Self::RightAlt),
            "super" | "meta" | "win" | "cmd" | "mod" | "lmeta" | "leftmeta" | "key_leftmeta" => {
                Some(Self::LeftMeta)
            }
            "rmeta" | "rightmeta" | "key_rightmeta" => Some(Self::RightMeta),
            "a" | "key_a" => Some(Self::A),
            "b" | "key_b" => Some(Self::B),
            "c" | "key_c" => Some(Self::C),
            "d" | "key_d" => Some(Self::D),
            "e" | "key_e" => Some(Self::E),
            "f" | "key_f" => Some(Self::F),
            "g" | "key_g" => Some(Self::G),
            "h" | "key_h" => Some(Self::H),
            "i" | "key_i" => Some(Self::I),
            "j" | "key_j" => Some(Self::J),
            "k" | "key_k" => Some(Self::K),
            "l" | "key_l" => Some(Self::L),
            "m" | "key_m" => Some(Self::M),
            "n" | "key_n" => Some(Self::N),
            "o" | "key_o" => Some(Self::O),
            "p" | "key_p" => Some(Self::P),
            "q" | "key_q" => Some(Self::Q),
            "r" | "key_r" => Some(Self::R),
            "s" | "key_s" => Some(Self::S),
            "t" | "key_t" => Some(Self::T),
            "u" | "key_u" => Some(Self::U),
            "v" | "key_v" => Some(Self::V),
            "w" | "key_w" => Some(Self::W),
            "x" | "key_x" => Some(Self::X),
            "y" | "key_y" => Some(Self::Y),
            "z" | "key_z" => Some(Self::Z),
            "0" | "key_0" => Some(Self::Num0),
            "1" | "key_1" => Some(Self::Num1),
            "2" | "key_2" => Some(Self::Num2),
            "3" | "key_3" => Some(Self::Num3),
            "4" | "key_4" => Some(Self::Num4),
            "5" | "key_5" => Some(Self::Num5),
            "6" | "key_6" => Some(Self::Num6),
            "7" | "key_7" => Some(Self::Num7),
            "8" | "key_8" => Some(Self::Num8),
            "9" | "key_9" => Some(Self::Num9),
            "f1" | "key_f1" => Some(Self::F1),
            "f2" | "key_f2" => Some(Self::F2),
            "f3" | "key_f3" => Some(Self::F3),
            "f4" | "key_f4" => Some(Self::F4),
            "f5" | "key_f5" => Some(Self::F5),
            "f6" | "key_f6" => Some(Self::F6),
            "f7" | "key_f7" => Some(Self::F7),
            "f8" | "key_f8" => Some(Self::F8),
            "f9" | "key_f9" => Some(Self::F9),
            "f10" | "key_f10" => Some(Self::F10),
            "f11" | "key_f11" => Some(Self::F11),
            "f12" | "key_f12" => Some(Self::F12),
            "tab" | "key_tab" => Some(Self::Tab),
            "enter" | "return" | "key_enter" => Some(Self::Enter),
            "esc" | "escape" | "key_esc" => Some(Self::Esc),
            "space" | "key_space" => Some(Self::Space),
            "backspace" | "bs" | "key_backspace" => Some(Self::Backspace),
            "delete" | "del" | "key_delete" => Some(Self::Delete),
            "insert" | "ins" | "key_insert" => Some(Self::Insert),
            "up" | "key_up" => Some(Self::Up),
            "down" | "key_down" => Some(Self::Down),
            "left" | "key_left" => Some(Self::Left),
            "right" | "key_right" => Some(Self::Right),
            "home" | "key_home" => Some(Self::Home),
            "end" | "key_end" => Some(Self::End),
            "pageup" | "pgup" | "key_pageup" => Some(Self::PageUp),
            "pagedown" | "pgdn" | "key_pagedown" => Some(Self::PageDown),
            "-" | "minus" | "key_minus" => Some(Self::Minus),
            "=" | "equal" | "equals" | "key_equal" => Some(Self::Equal),
            "[" | "leftbrace" | "lbracket" | "key_leftbrace" => Some(Self::LeftBrace),
            "]" | "rightbrace" | "rbracket" | "key_rightbrace" => Some(Self::RightBrace),
            ";" | "semicolon" | "key_semicolon" => Some(Self::Semicolon),
            "'" | "apostrophe" | "quote" | "key_apostrophe" => Some(Self::Apostrophe),
            "`" | "grave" | "backtick" | "key_grave" => Some(Self::Grave),
            "\\" | "backslash" | "key_backslash" => Some(Self::Backslash),
            "comma" | "key_comma" => Some(Self::Comma),
            "." | "dot" | "period" | "key_dot" => Some(Self::Dot),
            "/" | "slash" | "key_slash" => Some(Self::Slash),
            "capslock" | "key_capslock" => Some(Self::CapsLock),
            "print" | "printscreen" | "prtsc" | "key_print" => Some(Self::Print),
            "btn_left" => Some(Self::BtnLeft),
            "btn_right" => Some(Self::BtnRight),
            "btn_middle" => Some(Self::BtnMiddle),
            "playpause" | "key_playpause" => Some(Self::PlayPause),
            "nextsong" | "next" | "key_nextsong" => Some(Self::NextSong),
            "prevsong" | "previoussong" | "prev" | "key_previoussong" => Some(Self::PrevSong),
            "volumeup" | "key_volumeup" => Some(Self::VolumeUp),
            "volumedown" | "key_volumedown" => Some(Self::VolumeDown),
            "mute" | "key_mute" => Some(Self::Mute),
            "brightnessup" | "key_brightnessup" => Some(Self::BrightnessUp),
            "brightnessdown" | "key_brightnessdown" => Some(Self::BrightnessDown),
            _ => None,
        }
    }

    /// All keys registered on the Linux virtual keyboard.
    pub const ALL: &'static [MacroKey] = &[
        Self::LeftCtrl,
        Self::RightCtrl,
        Self::LeftShift,
        Self::RightShift,
        Self::LeftAlt,
        Self::RightAlt,
        Self::LeftMeta,
        Self::RightMeta,
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
        Self::H,
        Self::I,
        Self::J,
        Self::K,
        Self::L,
        Self::M,
        Self::N,
        Self::O,
        Self::P,
        Self::Q,
        Self::R,
        Self::S,
        Self::T,
        Self::U,
        Self::V,
        Self::W,
        Self::X,
        Self::Y,
        Self::Z,
        Self::Num0,
        Self::Num1,
        Self::Num2,
        Self::Num3,
        Self::Num4,
        Self::Num5,
        Self::Num6,
        Self::Num7,
        Self::Num8,
        Self::Num9,
        Self::F1,
        Self::F2,
        Self::F3,
        Self::F4,
        Self::F5,
        Self::F6,
        Self::F7,
        Self::F8,
        Self::F9,
        Self::F10,
        Self::F11,
        Self::F12,
        Self::Tab,
        Self::Enter,
        Self::Esc,
        Self::Space,
        Self::Backspace,
        Self::Delete,
        Self::Insert,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::Home,
        Self::End,
        Self::PageUp,
        Self::PageDown,
        Self::Minus,
        Self::Equal,
        Self::LeftBrace,
        Self::RightBrace,
        Self::Semicolon,
        Self::Apostrophe,
        Self::Grave,
        Self::Backslash,
        Self::Comma,
        Self::Dot,
        Self::Slash,
        Self::CapsLock,
        Self::Print,
        Self::BtnLeft,
        Self::BtnRight,
        Self::BtnMiddle,
        Self::PlayPause,
        Self::NextSong,
        Self::PrevSong,
        Self::VolumeUp,
        Self::VolumeDown,
        Self::Mute,
        Self::BrightnessUp,
        Self::BrightnessDown,
    ];
}

#[cfg(target_os = "linux")]
impl From<MacroKey> for KeyCode {
    fn from(k: MacroKey) -> Self {
        match k {
            MacroKey::LeftCtrl => KeyCode::KEY_LEFTCTRL,
            MacroKey::RightCtrl => KeyCode::KEY_RIGHTCTRL,
            MacroKey::LeftShift => KeyCode::KEY_LEFTSHIFT,
            MacroKey::RightShift => KeyCode::KEY_RIGHTSHIFT,
            MacroKey::LeftAlt => KeyCode::KEY_LEFTALT,
            MacroKey::RightAlt => KeyCode::KEY_RIGHTALT,
            MacroKey::LeftMeta => KeyCode::KEY_LEFTMETA,
            MacroKey::RightMeta => KeyCode::KEY_RIGHTMETA,
            MacroKey::A => KeyCode::KEY_A,
            MacroKey::B => KeyCode::KEY_B,
            MacroKey::C => KeyCode::KEY_C,
            MacroKey::D => KeyCode::KEY_D,
            MacroKey::E => KeyCode::KEY_E,
            MacroKey::F => KeyCode::KEY_F,
            MacroKey::G => KeyCode::KEY_G,
            MacroKey::H => KeyCode::KEY_H,
            MacroKey::I => KeyCode::KEY_I,
            MacroKey::J => KeyCode::KEY_J,
            MacroKey::K => KeyCode::KEY_K,
            MacroKey::L => KeyCode::KEY_L,
            MacroKey::M => KeyCode::KEY_M,
            MacroKey::N => KeyCode::KEY_N,
            MacroKey::O => KeyCode::KEY_O,
            MacroKey::P => KeyCode::KEY_P,
            MacroKey::Q => KeyCode::KEY_Q,
            MacroKey::R => KeyCode::KEY_R,
            MacroKey::S => KeyCode::KEY_S,
            MacroKey::T => KeyCode::KEY_T,
            MacroKey::U => KeyCode::KEY_U,
            MacroKey::V => KeyCode::KEY_V,
            MacroKey::W => KeyCode::KEY_W,
            MacroKey::X => KeyCode::KEY_X,
            MacroKey::Y => KeyCode::KEY_Y,
            MacroKey::Z => KeyCode::KEY_Z,
            MacroKey::Num0 => KeyCode::KEY_0,
            MacroKey::Num1 => KeyCode::KEY_1,
            MacroKey::Num2 => KeyCode::KEY_2,
            MacroKey::Num3 => KeyCode::KEY_3,
            MacroKey::Num4 => KeyCode::KEY_4,
            MacroKey::Num5 => KeyCode::KEY_5,
            MacroKey::Num6 => KeyCode::KEY_6,
            MacroKey::Num7 => KeyCode::KEY_7,
            MacroKey::Num8 => KeyCode::KEY_8,
            MacroKey::Num9 => KeyCode::KEY_9,
            MacroKey::F1 => KeyCode::KEY_F1,
            MacroKey::F2 => KeyCode::KEY_F2,
            MacroKey::F3 => KeyCode::KEY_F3,
            MacroKey::F4 => KeyCode::KEY_F4,
            MacroKey::F5 => KeyCode::KEY_F5,
            MacroKey::F6 => KeyCode::KEY_F6,
            MacroKey::F7 => KeyCode::KEY_F7,
            MacroKey::F8 => KeyCode::KEY_F8,
            MacroKey::F9 => KeyCode::KEY_F9,
            MacroKey::F10 => KeyCode::KEY_F10,
            MacroKey::F11 => KeyCode::KEY_F11,
            MacroKey::F12 => KeyCode::KEY_F12,
            MacroKey::Tab => KeyCode::KEY_TAB,
            MacroKey::Enter => KeyCode::KEY_ENTER,
            MacroKey::Esc => KeyCode::KEY_ESC,
            MacroKey::Space => KeyCode::KEY_SPACE,
            MacroKey::Backspace => KeyCode::KEY_BACKSPACE,
            MacroKey::Delete => KeyCode::KEY_DELETE,
            MacroKey::Insert => KeyCode::KEY_INSERT,
            MacroKey::Up => KeyCode::KEY_UP,
            MacroKey::Down => KeyCode::KEY_DOWN,
            MacroKey::Left => KeyCode::KEY_LEFT,
            MacroKey::Right => KeyCode::KEY_RIGHT,
            MacroKey::Home => KeyCode::KEY_HOME,
            MacroKey::End => KeyCode::KEY_END,
            MacroKey::PageUp => KeyCode::KEY_PAGEUP,
            MacroKey::PageDown => KeyCode::KEY_PAGEDOWN,
            MacroKey::Minus => KeyCode::KEY_MINUS,
            MacroKey::Equal => KeyCode::KEY_EQUAL,
            MacroKey::LeftBrace => KeyCode::KEY_LEFTBRACE,
            MacroKey::RightBrace => KeyCode::KEY_RIGHTBRACE,
            MacroKey::Semicolon => KeyCode::KEY_SEMICOLON,
            MacroKey::Apostrophe => KeyCode::KEY_APOSTROPHE,
            MacroKey::Grave => KeyCode::KEY_GRAVE,
            MacroKey::Backslash => KeyCode::KEY_BACKSLASH,
            MacroKey::Comma => KeyCode::KEY_COMMA,
            MacroKey::Dot => KeyCode::KEY_DOT,
            MacroKey::Slash => KeyCode::KEY_SLASH,
            MacroKey::CapsLock => KeyCode::KEY_CAPSLOCK,
            MacroKey::Print => KeyCode::KEY_PRINT,
            MacroKey::BtnLeft => KeyCode::BTN_LEFT,
            MacroKey::BtnRight => KeyCode::BTN_RIGHT,
            MacroKey::BtnMiddle => KeyCode::BTN_MIDDLE,
            MacroKey::PlayPause => KeyCode::KEY_PLAYPAUSE,
            MacroKey::NextSong => KeyCode::KEY_NEXTSONG,
            MacroKey::PrevSong => KeyCode::KEY_PREVIOUSSONG,
            MacroKey::VolumeUp => KeyCode::KEY_VOLUMEUP,
            MacroKey::VolumeDown => KeyCode::KEY_VOLUMEDOWN,
            MacroKey::Mute => KeyCode::KEY_MUTE,
            MacroKey::BrightnessUp => KeyCode::KEY_BRIGHTNESSUP,
            MacroKey::BrightnessDown => KeyCode::KEY_BRIGHTNESSDOWN,
        }
    }
}

/// A single key chord: zero or more modifiers held while tapping `key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCombo {
    pub modifiers: Vec<MacroKey>,
    pub key: MacroKey,
}

/// Parses a single chord such as `"ctrl+shift+tab"` into a [`KeyCombo`].
pub fn parse_combo(expr: &str) -> Result<KeyCombo, String> {
    let parts: Vec<&str> = expr
        .split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return Err("empty key combo".into());
    }
    let mut modifiers = Vec::new();
    let mut main_key = None;
    for (idx, part) in parts.iter().enumerate() {
        let k = MacroKey::parse(part).ok_or_else(|| format!("unknown key '{part}'"))?;
        if idx + 1 < parts.len() {
            if k.is_modifier() {
                modifiers.push(k);
            } else {
                return Err(format!("non-modifier key '{part}' must be last in combo"));
            }
        } else {
            main_key = Some(k);
        }
    }
    Ok(KeyCombo {
        modifiers,
        key: main_key.ok_or_else(|| "missing key in combo".to_string())?,
    })
}

/// Parses a comma-separated sequence of chords, e.g. `"ctrl+l, enter"`.
pub fn parse_sequence(expr: &str) -> Result<Vec<KeyCombo>, String> {
    let mut steps = Vec::new();
    for part in expr.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        steps.push(parse_combo(part)?);
    }
    if steps.is_empty() {
        return Err("empty macro sequence".into());
    }
    Ok(steps)
}

/// Returns true if `apps` is empty (global) or any entry matches `active` (case-insensitive substring).
pub fn app_matches(apps: &[String], active: Option<&str>) -> bool {
    let non_empty: Vec<&str> = apps
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if non_empty.is_empty() {
        return true;
    }
    let Some(win) = active else {
        return false;
    };
    let win_lower = win.to_ascii_lowercase();
    non_empty
        .iter()
        .any(|pat| win_lower.contains(&pat.to_ascii_lowercase()))
}

/// Best-effort detection of the focused window's class/app_id/title on Linux.
pub fn active_window_info() -> Option<String> {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
        && let Some(info) = hyprland_active_window()
    {
        return Some(info);
    }
    if std::env::var_os("SWAYSOCK").is_some()
        && let Some(info) = sway_i3_active_window("swaymsg")
    {
        return Some(info);
    }
    if std::env::var_os("I3SOCK").is_some()
        && let Some(info) = sway_i3_active_window("i3-msg")
    {
        return Some(info);
    }
    let de = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_uppercase();
    if (de.contains("KDE") || de.contains("PLASMA"))
        && let Some(info) = kdotool_active_window()
    {
        return Some(info);
    }
    if std::env::var_os("DISPLAY").is_some()
        && let Some(info) = x11_active_window()
    {
        return Some(info);
    }
    None
}

fn hyprland_active_window() -> Option<String> {
    let out = Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let parts: Vec<&str> = [
        v["class"].as_str(),
        v["initialClass"].as_str(),
        v["title"].as_str(),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn sway_i3_active_window(bin: &str) -> Option<String> {
    let out = Command::new(bin).args(["-t", "get_tree"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    find_focused_sway_node(&v)
}

fn find_focused_sway_node(node: &serde_json::Value) -> Option<String> {
    if node["focused"].as_bool() == Some(true) {
        let parts: Vec<&str> = [
            node["app_id"].as_str(),
            node["window_properties"]["class"].as_str(),
            node["window_properties"]["instance"].as_str(),
            node["name"].as_str(),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect();
        if !parts.is_empty() {
            return Some(parts.join(" "));
        }
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(arr) = node[key].as_array() {
            for child in arr {
                if let Some(found) = find_focused_sway_node(child) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn kdotool_active_window() -> Option<String> {
    let out = Command::new("kdotool")
        .args(["getactivewindow", "getwindowclassname", "getwindowname"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn x11_active_window() -> Option<String> {
    let root = Command::new("xprop")
        .args(["-root", "_NET_ACTIVE_WINDOW"])
        .output()
        .ok()?;
    if !root.status.success() {
        return None;
    }
    let root_str = String::from_utf8_lossy(&root.stdout);
    let win_id = root_str
        .split_whitespace()
        .find(|tok| tok.starts_with("0x"))?;
    if win_id == "0x0" {
        return None;
    }
    let props = Command::new("xprop")
        .args(["-id", win_id, "WM_CLASS", "WM_NAME"])
        .output()
        .ok()?;
    if !props.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&props.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combos_and_sequences() {
        let combo = parse_combo("Ctrl+Tab").unwrap();
        assert_eq!(combo.modifiers, vec![MacroKey::LeftCtrl]);
        assert_eq!(combo.key, MacroKey::Tab);

        let seq = parse_sequence("ctrl+shift+t, super+1").unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(
            seq[0].modifiers,
            vec![MacroKey::LeftCtrl, MacroKey::LeftShift]
        );
        assert_eq!(seq[0].key, MacroKey::T);
        assert_eq!(seq[1].modifiers, vec![MacroKey::LeftMeta]);
        assert_eq!(seq[1].key, MacroKey::Num1);
    }

    #[test]
    fn rejects_invalid_combos() {
        assert!(parse_combo("").is_err());
        assert!(parse_combo("a+b").is_err());
        assert!(parse_combo("ctrl+notakey").is_err());
    }

    #[test]
    fn app_matching_is_case_insensitive_substring() {
        assert!(app_matches(&[], None));
        assert!(!app_matches(&["firefox".into()], None));
        assert!(app_matches(
            &["firefox".into(), "chromium".into()],
            Some("org.mozilla.Firefox — GitHub")
        ));
        assert!(!app_matches(&["firefox".into()], Some("konsole")));
    }
}
