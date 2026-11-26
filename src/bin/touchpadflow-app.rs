#![cfg_attr(windows, windows_subsystem = "windows")] // no console window behind the app

//! TouchpadFlow settings app: native window (egui), no browser, no network.
//! Edits the shared config file; every engine (Linux daemon, macOS, Windows) reads it.
//! Look: monochrome "ink on paper" (see DESIGN.md), dark mode prints it in negative.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Painter,
    Pos2, Rect, Response, Role, Sense, Shadow, Stroke, StrokeKind, TextStyle, Ui, Vec2, Visuals,
    WidgetInfo, style::HandleShape,
};
use touchpadflow::config::{
    AppConfig, EDGE_RATIO_RANGE, EdgeAction, GESTURE_NAMES, MacroRule, SENSITIVITY_RANGE,
    ThemeMode, config_path,
};
use touchpadflow::macros::parse_sequence;

const AUTOSAVE_DELAY: Duration = Duration::from_millis(350);
const BORDER: f32 = 2.0;
const OFFSET: f32 = 3.0; // hard "press" shadow under buttons

fn main() -> eframe::Result {
    // wgpu (default) can fail on VMs, RDP and old GPU drivers; the Windows build has no
    // console, so that used to look like the app "never launching". Fall back to OpenGL.
    let first = run(eframe::Renderer::Wgpu);
    let Err(wgpu_err) = first else { return first };
    eprintln!("wgpu renderer failed ({wgpu_err}); retrying with OpenGL");
    let second = run(eframe::Renderer::Glow);
    if let Err(e) = &second {
        let msg = format!("wgpu: {wgpu_err}\nglow: {e}\n");
        if let Some(dir) = config_path().parent() {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(dir.join("touchpadflow-app.log"), msg);
        }
    }
    second
}

fn run(renderer: eframe::Renderer) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TouchpadFlow")
            .with_app_id("touchpadflow")
            .with_inner_size([660.0, 900.0])
            .with_min_inner_size([480.0, 600.0]),
        renderer,
        ..Default::default()
    };
    eframe::run_native(
        "TouchpadFlow",
        options,
        Box::new(|cc| Ok(Box::new(SettingsApp::new(&cc.egui_ctx)))),
    )
}

// ---------------------------------------------------------------- theme

/// Four tones like a 2-bit print, plus one accent. Dark swaps the tones end for end;
/// only Sumi gives the accent its own colour.
#[derive(Clone, Copy, PartialEq)]
struct Tones {
    ink: Color32,
    ink_soft: Color32,
    rule: Color32,
    paper: Color32,
    accent: Color32,
}

const C0: Color32 = Color32::from_rgb(0x0c, 0x0c, 0x0b);
const C1: Color32 = Color32::from_rgb(0x4a, 0x4a, 0x48);
const C2: Color32 = Color32::from_rgb(0x9b, 0x9b, 0x98);
const C3: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe2);

impl Tones {
    const LIGHT: Self = Self {
        ink: C0,
        ink_soft: C1,
        rule: C2,
        paper: C3,
        accent: C0,
    };
    const DARK: Self = Self {
        ink: C3,
        ink_soft: C2,
        rule: C1,
        paper: C0,
        accent: C3,
    };
    /// Warm washi paper, brown-black sumi ink, and a vermilion hanko-seal red.
    const SUMI: Self = Self {
        ink: Color32::from_rgb(0x22, 0x1e, 0x19),
        ink_soft: Color32::from_rgb(0x5c, 0x53, 0x48),
        rule: Color32::from_rgb(0xb0, 0xa3, 0x8c),
        paper: Color32::from_rgb(0xec, 0xe3, 0xcf),
        accent: Color32::from_rgb(0xb4, 0x32, 0x1e),
    };

    fn of(dark: bool) -> Self {
        if dark { Self::DARK } else { Self::LIGHT }
    }
    /// The tones chosen for this frame (see `SettingsApp::ui`), else light/dark from visuals.
    fn ui(ui: &Ui) -> Self {
        ui.data(|d| d.get_temp(egui::Id::NULL))
            .unwrap_or_else(|| Self::of(ui.visuals().dark_mode))
    }
}

fn display() -> FontFamily {
    FontFamily::Name("display".into())
}
fn strong() -> FontFamily {
    FontFamily::Name("strong".into())
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (
            "plex",
            &include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf")[..],
        ),
        (
            "plex-semibold",
            &include_bytes!("../../assets/fonts/IBMPlexMono-SemiBold.ttf")[..],
        ),
        (
            "space-mono-bold",
            &include_bytes!("../../assets/fonts/SpaceMono-Bold.ttf")[..],
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    // Keep egui's bundled fonts as fallback for glyphs the mono faces lack.
    for (family, first) in [
        (FontFamily::Proportional, "plex"),
        (FontFamily::Monospace, "plex"),
    ] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, first.into());
    }
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    for (family, first) in [(display(), "space-mono-bold"), (strong(), "plex-semibold")] {
        let mut list = vec![first.to_string()];
        list.extend(fallback.iter().cloned());
        fonts.families.insert(family, list);
    }
    ctx.set_fonts(fonts);
}

fn visuals(t: Tones, dark: bool) -> Visuals {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = t.paper;
    v.window_fill = t.paper;
    v.extreme_bg_color = t.paper;
    v.faint_bg_color = t.paper;
    v.code_bg_color = t.paper;
    v.window_stroke = Stroke::new(BORDER, t.ink);
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    let hard = Shadow {
        offset: [3, 3],
        blur: 0,
        spread: 0,
        color: t.ink,
    };
    v.popup_shadow = hard;
    v.window_shadow = hard;
    v.selection.bg_fill = t.accent; // also the filled part of slider rails
    v.selection.stroke = Stroke::new(1.0, t.paper);
    v.hyperlink_color = t.ink;
    v.warn_fg_color = t.ink;
    v.error_fg_color = t.ink;
    v.text_cursor.stroke = Stroke::new(2.0, t.ink);
    v.handle_shape = HandleShape::Rect { aspect_ratio: 0.55 };
    v.slider_trailing_fill = true;
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = false;

    let ink_line = Stroke::new(BORDER, t.ink);
    let w = &mut v.widgets;
    // bg_fill paints slider rails and handles, weak_bg_fill paints buttons and combo boxes.
    for (wv, bg, weak, stroke, fg) in [
        (
            &mut w.noninteractive,
            t.paper,
            t.paper,
            Stroke::new(1.0, t.rule),
            t.ink,
        ),
        (&mut w.inactive, t.rule, t.paper, ink_line, t.ink),
        (&mut w.hovered, t.ink_soft, t.rule, ink_line, t.ink),
        (&mut w.active, t.ink, t.ink, ink_line, t.paper),
        (&mut w.open, t.rule, t.paper, ink_line, t.ink),
    ] {
        wv.bg_fill = bg;
        wv.weak_bg_fill = weak;
        wv.bg_stroke = stroke;
        wv.fg_stroke = Stroke::new(1.5, fg);
        wv.corner_radius = CornerRadius::ZERO;
        wv.expansion = 0.0;
    }
    v
}

fn to_egui_theme(mode: ThemeMode) -> egui::ThemePreference {
    match mode {
        ThemeMode::System => egui::ThemePreference::System,
        ThemeMode::Light | ThemeMode::Sumi => egui::ThemePreference::Light,
        ThemeMode::Dark => egui::ThemePreference::Dark,
    }
}

fn apply_theme(ctx: &egui::Context, mode: ThemeMode) {
    install_fonts(ctx);
    // Follows the configured/system theme; TOUCHPADFLOW_THEME=light|dark pins one.
    let pref = match std::env::var("TOUCHPADFLOW_THEME").as_deref() {
        Ok("light") => egui::ThemePreference::Light,
        Ok("dark") => egui::ThemePreference::Dark,
        _ => to_egui_theme(mode),
    };
    ctx.set_theme(pref);
    let light = if mode == ThemeMode::Sumi {
        Tones::SUMI
    } else {
        Tones::LIGHT
    };
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let dark = theme == egui::Theme::Dark;
        ctx.set_visuals_of(theme, visuals(if dark { Tones::DARK } else { light }, dark));
        ctx.style_mut_of(theme, |style| {
            use FontFamily::Proportional;
            style.text_styles = [
                (TextStyle::Heading, FontId::new(26.0, display())),
                (TextStyle::Body, FontId::new(13.5, Proportional)),
                (TextStyle::Button, FontId::new(13.0, Proportional)),
                (TextStyle::Small, FontId::new(11.0, Proportional)),
                (
                    TextStyle::Monospace,
                    FontId::new(13.0, FontFamily::Monospace),
                ),
            ]
            .into();
            style.spacing.item_spacing = Vec2::new(10.0, 8.0);
            style.spacing.interact_size.y = 30.0;
            style.spacing.button_padding = Vec2::new(10.0, 5.0);
            style.spacing.slider_rail_height = 4.0;
        });
    }
}

// ---------------------------------------------------------------- app state

#[derive(Clone, PartialEq)]
enum Engine {
    Checking,
    Running {
        paused: bool,
        device: String,
    },
    NotRunning,
    /// macOS/Windows engines have no control socket; they re-read the file.
    WatchesFile,
}

enum Notice {
    Info(String),
    Error(String),
}

struct SettingsApp {
    path: PathBuf,
    cfg: AppConfig,
    saved: AppConfig,
    dirty_since: Option<Instant>,
    notice: Option<Notice>,
    engine: Arc<Mutex<Engine>>,
    recording_macro: Option<usize>,
    backdrop: Backdrop,
}

impl SettingsApp {
    fn new(ctx: &egui::Context) -> Self {
        let path = config_path();
        let (cfg, notice) = match AppConfig::load(&path) {
            Ok(c) => (c, None),
            Err(e) => (
                AppConfig::default(),
                Some(Notice::Error(format!(
                    "{e}. Showing defaults; your next change will replace the file."
                ))),
            ),
        };
        apply_theme(ctx, cfg.theme);
        let engine = Arc::new(Mutex::new(Engine::Checking));
        spawn_status_poller(ctx.clone(), Arc::clone(&engine));
        Self {
            path,
            saved: cfg.clone(),
            cfg,
            dirty_since: None,
            notice,
            engine,
            recording_macro: None,
            backdrop: Backdrop::default(),
        }
    }

    fn engine(&self) -> Engine {
        self.engine
            .lock()
            .map(|e| e.clone())
            .unwrap_or(Engine::Checking)
    }

    fn save_now(&mut self) {
        self.cfg = self.cfg.clone().sanitized();
        self.dirty_since = None;
        if let Err(e) = self.cfg.save(&self.path) {
            self.notice = Some(Notice::Error(format!(
                "Could not save {}: {e}",
                self.path.display()
            )));
            return;
        }
        self.saved = self.cfg.clone();
        self.notice = Some(match notify_engine() {
            Ok(()) => Notice::Info("Saved and applied.".into()),
            Err(hint) => Notice::Info(format!("Saved. {hint}")),
        });
    }
}

#[cfg(unix)]
fn notify_engine() -> Result<(), String> {
    match touchpadflow::ipc::send_command("reload") {
        Ok(_) => Ok(()),
        Err(_) if cfg!(target_os = "macos") => {
            Err("The macOS engine reloads it within a second.".into())
        }
        Err(_) => Err(
            "Engine is not running; start it with: systemctl --user enable --now touchpadflow"
                .into(),
        ),
    }
}

#[cfg(not(unix))]
fn notify_engine() -> Result<(), String> {
    Err("The Windows engine reloads it within a second.".into())
}

const INSTALL_SH: &str =
    "curl -fsSL https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.sh | bash";
const INSTALL_PS1: &str =
    "irm https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.ps1 | iex";

/// Runs the same one-liner as a fresh install, in a visible terminal so the user sees
/// progress and can type a sudo password. Returns once the terminal has been opened.
fn launch_updater() -> Result<(), String> {
    use std::process::Command;
    let spawned = if cfg!(windows) {
        // `start` gives the installer its own console window.
        Command::new("cmd")
            .args([
                "/C",
                "start",
                "TouchpadFlow update",
                "powershell",
                "-NoProfile",
            ])
            .args(["-ExecutionPolicy", "Bypass", "-Command", INSTALL_PS1])
            .spawn()
            .map(drop)
    } else if cfg!(target_os = "macos") {
        let script = format!("tell application \"Terminal\" to do script \"{INSTALL_SH}\"");
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map(drop)
    } else {
        let cmd = format!("{INSTALL_SH}; echo; read -rp 'Done. Press Enter to close.' _");
        let terminals: [(&str, &[&str]); 5] = [
            ("x-terminal-emulator", &["-e"]),
            ("konsole", &["-e"]),
            ("gnome-terminal", &["--"]),
            ("kitty", &[]),
            ("xterm", &["-e"]),
        ];
        let mut last = Err(std::io::Error::other("no terminal emulator found"));
        for (bin, args) in terminals {
            last = Command::new(bin)
                .args(args)
                .args(["bash", "-c", &cmd])
                .spawn()
                .map(drop);
            if last.is_ok() {
                break;
            }
        }
        last
    };
    spawned.map_err(|e| e.to_string())
}

fn spawn_status_poller(ctx: egui::Context, slot: Arc<Mutex<Engine>>) {
    std::thread::spawn(move || {
        loop {
            let next = probe_engine();
            if let Ok(mut cur) = slot.lock()
                && *cur != next
            {
                *cur = next;
                ctx.request_repaint();
            }
            std::thread::sleep(Duration::from_millis(1500));
        }
    });
}

fn probe_engine() -> Engine {
    if !cfg!(target_os = "linux") {
        return Engine::WatchesFile;
    }
    #[cfg(unix)]
    if let Ok(resp) = touchpadflow::ipc::send_command("status") {
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap_or_default();
        return Engine::Running {
            paused: v["paused"].as_bool().unwrap_or(false),
            device: v["device"].as_str().unwrap_or("").to_string(),
        };
    }
    Engine::NotRunning
}

// ---------------------------------------------------------------- background

// Procedural ink page. Rendered once into a texture (not per frame, not vector shapes) so it
// gets what real ink has: feathered edges, halftone, grain and grey wash.

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_215.0
}

/// Smooth value noise in 0..1.
fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (fx, fy) = (x - xf, y - yf);
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (i, j) = (xf as i32, yf as i32);
    let top = hash2(i, j, seed) + (hash2(i + 1, j, seed) - hash2(i, j, seed)) * ux;
    let bot = hash2(i, j + 1, seed) + (hash2(i + 1, j + 1, seed) - hash2(i, j + 1, seed)) * ux;
    top + (bot - top) * uy
}

/// Fractal noise in 0..1: cloud-like, the basis for smoke, wash and bleed.
fn fbm(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 0.5, 0.0, 1.0);
    for o in 0..octaves {
        sum += amp * vnoise(x * f, y * f, seed + o * 17);
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / norm
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Anti-aliased coverage of a shape whose signed edge distance is `d` px (positive = inside).
fn cover(d: f32) -> f32 {
    (d + 0.5).clamp(0.0, 1.0)
}

/// Ink coverage (0 = bare paper, 1 = solid ink) at logical point (x, y) of a w x h page.
/// Restrained on purpose: bare paper in the middle, ink only gathered in two corners.
fn ink_at(x: f32, y: f32, w: f32, h: f32) -> f32 {
    let edge_d = x.min(w - x).min(y).min(h - y);
    let mut a: f32 = 0.0;

    // Paper: faint fibres and the odd fleck of dust.
    a += 0.03 * (vnoise(x / 2.2, y / 11.0, 2) - 0.5);
    if hash2(x as i32, y as i32, 1) > 0.998 {
        a = a.max(0.4);
    }

    // Corner weights: ink lives top-right and bottom-left only.
    let tr = (1.0 - ((x - w).powi(2) + y * y).sqrt() / 240.0).clamp(0.0, 1.0);
    // Bottom-left accent sits above the footer so the settings path stays readable.
    let bl = (1.0 - (x * x + (y - (h - 200.0)).powi(2)).sqrt() / 150.0).clamp(0.0, 1.0);

    // Screentone smoke in those corners: 45-degree halftone dots sized by a cloud field.
    let smoke = smoothstep(0.45, 0.75, fbm(x / 120.0, y / 100.0, 4, 4)) * tr.max(bl).powf(1.5);
    if smoke > 0.02 {
        const CELL: f32 = 4.6;
        let u = (x + y) * std::f32::consts::FRAC_1_SQRT_2 / CELL;
        let v = (x - y) * std::f32::consts::FRAC_1_SQRT_2 / CELL;
        let (du, dv) = (u - u.floor() - 0.5, v - v.floor() - 0.5);
        let dist = (du * du + dv * dv).sqrt() * CELL;
        a = a.max(cover(smoke.sqrt() * CELL * 0.6 - dist) * 0.85);
    }

    // Hatching: a few diagonal pen strokes in the bottom-left corner.
    if bl > 0.0 {
        const PERIOD: f32 = 5.0;
        let s = (x * 0.5 + y * 0.866) / PERIOD;
        let line = s.floor();
        let along = -x * 0.866 + y * 0.5;
        let gate = vnoise(line * 0.61, along / 46.0, 11);
        let present = smoothstep(gate - 0.08, gate + 0.08, bl * 0.9);
        a = a.max(cover(0.5 - (s - line - 0.5).abs() * PERIOD) * present * 0.8);
    }

    // A small cluster of ink running down from the top-right corner.
    let cr = (1.0 - (w - x) / (w * 0.3)).clamp(0.0, 1.0);
    a = a.max(cover((2.0 + 10.0 * fbm(x / 22.0, 0.5, 3, 23)) * cr - y));
    const COL: f32 = 9.0;
    let ci = (x / COL).floor() as i32;
    for k in ci - 2..=ci + 2 {
        let cx = (k as f32 + hash2(k, 1, 21)) * COL;
        let cc = (1.0 - (w - cx) / (w * 0.3)).clamp(0.0, 1.0);
        if cc <= 0.0 || hash2(k, 0, 21) > 0.2 + 0.6 * cc {
            continue;
        }
        let len = (6.0 + hash2(k, 2, 21).powi(3) * 150.0) * cc;
        let wd = (0.8 + hash2(k, 3, 21) * 1.6) * (0.6 + cc * 0.6);
        if y > len + wd * 3.0 {
            continue;
        }
        let dx = (x - cx).abs();
        let rough = 1.0 + 0.3 * (vnoise(k as f32 * 3.1, y / 5.0, 22) - 0.5);
        let root = (12.0 - y).max(0.0).powi(2) * 0.02;
        let half = wd * (1.0 - 0.65 * (y / len).clamp(0.0, 1.0).sqrt()) * rough + root;
        let body = if y <= len { cover(half - dx) } else { 0.0 };
        let drop = cover(wd * 1.4 - ((x - cx).powi(2) + (y - len).powi(2)).sqrt());
        let feather = (-(dx - half).max(0.0) / 2.0).exp() * 0.2 * f32::from(u8::from(y < len));
        a = a.max(body.max(drop)).max(feather);
    }

    // Frame: one firm border.
    a = a.max(cover(3.0 - edge_d));

    a.clamp(0.0, 1.0)
}

/// Rasterises the ink page at `size` device pixels (`scale` px per point) on all cores.
fn ink_page(size: [usize; 2], scale: f32, t: Tones) -> egui::ColorImage {
    let [w, h] = size;
    let (lw, lh) = (w as f32 / scale, h as f32 / scale);
    let mut px = vec![t.paper; w * h];
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let rows = h.div_ceil(threads).max(1);
    let mix = |a: f32, p: u8, i: u8| (f32::from(p) + (f32::from(i) - f32::from(p)) * a) as u8;
    std::thread::scope(|s| {
        for (band, chunk) in px.chunks_mut(rows * w).enumerate() {
            s.spawn(move || {
                for (i, c) in chunk.iter_mut().enumerate() {
                    let x = (i % w) as f32 / scale;
                    let y = (band * rows + i / w) as f32 / scale;
                    let a = ink_at(x, y, lw, lh);
                    *c = Color32::from_rgb(
                        mix(a, t.paper.r(), t.ink.r()),
                        mix(a, t.paper.g(), t.ink.g()),
                        mix(a, t.paper.b(), t.ink.b()),
                    );
                }
            });
        }
    });
    egui::ColorImage::new(size, px)
}

/// The ink page texture, rebuilt when the window size or theme changes.
#[derive(Default)]
struct Backdrop {
    tex: Option<(egui::TextureHandle, [usize; 2], Tones)>,
    want: Option<([usize; 2], Instant)>,
}

impl Backdrop {
    fn paint(&mut self, ui: &Ui, rect: Rect, t: Tones) {
        let scale = ui.ctx().pixels_per_point().min(2.0);
        let size = [
            ((rect.width() * scale) as usize).max(1),
            ((rect.height() * scale) as usize).max(1),
        ];
        let current = self.tex.as_ref().map(|(_, s, d)| (*s, *d));
        if current != Some((size, t)) {
            // Mid-resize: stretch the old page and rebuild once the size holds for 150 ms.
            if self.want.is_none_or(|(s, _)| s != size) {
                self.want = Some((size, Instant::now()));
            }
            let since = self.want.map_or_else(Instant::now, |(_, at)| at);
            let settle = current.is_none_or(|(s, _)| s == size);
            if settle || since.elapsed() > Duration::from_millis(150) {
                let img = ink_page(size, scale, t);
                let tex = ui
                    .ctx()
                    .load_texture("ink-page", img, egui::TextureOptions::LINEAR);
                self.tex = Some((tex, size, t));
                self.want = None;
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(160));
            }
        }
        if let Some((tex, ..)) = &self.tex {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            ui.painter().image(tex.id(), rect, uv, Color32::WHITE);
        }
    }
}

/// Parallelogram (manga caption plate) leaning right by `skew`.
fn parallelogram(p: &Painter, r: Rect, skew: f32, fill: Color32, stroke: Stroke) {
    p.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(r.left() + skew, r.top()),
            Pos2::new(r.right(), r.top()),
            Pos2::new(r.right() - skew, r.bottom()),
            Pos2::new(r.left(), r.bottom()),
        ],
        fill,
        stroke,
    ));
}

fn italic_galley(p: &Painter, text: &str, font: FontId, color: Color32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.into(), font, color);
    job.sections[0].format.italics = true;
    p.layout_job(job)
}

/// Caption plate filled with `fill`, italic paper-coloured text.
fn tag(ui: &mut Ui, text: &str, font: FontId, fill: Color32) {
    let t = Tones::ui(ui);
    let galley = italic_galley(ui.painter(), text, font, t.paper);
    let (rect, _) = ui.allocate_exact_size(galley.size() + Vec2::new(36.0, 12.0), Sense::hover());
    let r = rect.shrink2(Vec2::new(3.0, 2.0));
    parallelogram(ui.painter(), r, 8.0, fill, Stroke::NONE);
    ui.painter()
        .galley(r.center() - galley.size() / 2.0, galley, t.paper);
}

// ---------------------------------------------------------------- ui

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save_now();
        }
        let before = self.cfg.clone();
        let tones = if self.cfg.theme == ThemeMode::Sumi {
            Tones::SUMI
        } else {
            Tones::of(ui.visuals().dark_mode)
        };
        ui.data_mut(|d| d.insert_temp(egui::Id::NULL, tones));

        // One ink page behind both panels, fixed to the window so it never scrolls.
        let page = ui.ctx().content_rect();
        if self.cfg.ink_page {
            self.backdrop.paint(ui, page, tones);
        } else {
            ui.painter().rect_filled(page, 0, tones.paper);
            let border = Stroke::new(3.0, tones.ink);
            ui.painter()
                .rect_stroke(page, 0, border, StrokeKind::Inside);
        }
        let margin = egui::Frame::new().inner_margin(egui::Margin::symmetric(22, 14));

        egui::Panel::bottom("footer")
            .frame(margin)
            .show(ui, |ui| self.footer(ui));

        egui::CentralPanel::default().frame(margin).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.header(ui);
                ui.add_space(18.0);
                ui.add_enabled_ui(self.cfg.enabled, |ui| {
                    section(ui, "01", "Touchpad map");
                    touchpad_map(ui, &mut self.cfg);
                    ui.add_space(18.0);
                    ui.columns(2, |cols| {
                        let ink = Stroke::new(2.0, Tones::ui(&cols[0]).ink);
                        let paper = Tones::ui(&cols[0]).paper;
                        let boxed = || egui::Frame::new().fill(paper).stroke(ink).inner_margin(8);
                        boxed().show(&mut cols[0], |ui| {
                            section(ui, "02", "Left edge");
                            zone_controls(
                                ui,
                                "left",
                                &mut self.cfg.left_action,
                                &mut self.cfg.left_edge_ratio,
                                &mut self.cfg.left_sensitivity,
                            );
                        });
                        boxed().show(&mut cols[1], |ui| {
                            section(ui, "03", "Right edge");
                            zone_controls(
                                ui,
                                "right",
                                &mut self.cfg.right_action,
                                &mut self.cfg.right_edge_ratio,
                                &mut self.cfg.right_sensitivity,
                            );
                        });
                    });
                    ui.add_space(14.0);
                    self.macros_section(ui);
                    ui.add_space(14.0);
                    self.advanced(ui);
                });
                ui.add_space(14.0);
                self.look(ui);
            });
        });

        // Auto-apply: wait until the user pauses (and is not mid-drag) before writing.
        if self.cfg != before {
            self.dirty_since = Some(Instant::now());
            self.notice = None;
        }
        if let Some(t) = self.dirty_since {
            let dragging = ui.input(|i| i.pointer.any_down());
            if !dragging && t.elapsed() >= AUTOSAVE_DELAY && self.cfg != self.saved {
                self.save_now();
            } else if self.cfg == self.saved {
                self.dirty_since = None;
            } else {
                ui.ctx().request_repaint_after(AUTOSAVE_DELAY);
            }
        }
    }
}

impl SettingsApp {
    fn header(&mut self, ui: &mut Ui) {
        let t = Tones::ui(ui);
        ui.horizontal(|ui| {
            let ink = Tones::ui(ui).ink;
            tag(ui, "TOUCHPADFLOW", FontId::new(28.0, display()), ink);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if self.cfg.enabled {
                    "GESTURES  ON"
                } else {
                    "GESTURES  OFF"
                };
                let resp = ink_button(ui, label, self.cfg.enabled)
                    .on_hover_text("Off hands the touchpad straight back to the system");
                resp.widget_info(|| {
                    WidgetInfo::selected(Role::CheckBox, true, self.cfg.enabled, "Gestures")
                });
                if resp.clicked() {
                    self.cfg.enabled = !self.cfg.enabled;
                }
            });
        });
        ui.add_space(8.0);

        let (glyph, text) = match self.engine() {
            Engine::Checking => (Glyph::Hollow, "CHECKING ENGINE".to_string()),
            Engine::Running { paused: true, .. } => (Glyph::Hollow, "ENGINE PAUSED".into()),
            Engine::Running { .. } if !self.cfg.enabled => {
                (Glyph::Hollow, "ENGINE IDLE / GESTURES OFF".into())
            }
            Engine::Running { device, .. } if device.is_empty() => {
                (Glyph::Solid, "ENGINE RUNNING".into())
            }
            Engine::Running { device, .. } => {
                (Glyph::Solid, format!("ENGINE RUNNING  /  {device}"))
            }
            Engine::NotRunning => (Glyph::Cross, "ENGINE NOT RUNNING".into()),
            Engine::WatchesFile => (Glyph::Solid, "SETTINGS APPLY TO THE PLATFORM ENGINE".into()),
        };
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
            glyph.paint(ui.painter(), r.shrink(1.0), t.ink);
            ui.label(
                egui::RichText::new(text)
                    .font(FontId::new(12.0, strong()))
                    .color(t.ink),
            );
        });
    }

    fn macros_section(&mut self, ui: &mut Ui) {
        let t = Tones::ui(ui);
        section(ui, "04", "Macros");
        ui.label(
            egui::RichText::new(
                "Bind gestures to key combos (ctrl+tab, ctrl+l, enter) or commands (command:...), scoped to specific apps or global.",
            )
            .size(11.5)
            .color(t.ink_soft),
        );
        ui.add_space(6.0);

        if let Some(rec_idx) = self.recording_macro {
            if rec_idx >= self.cfg.macros.len() {
                self.recording_macro = None;
            } else if let Some(captured) = capture_key_combo(ui) {
                if captured == "esc" {
                    self.recording_macro = None;
                } else {
                    self.cfg.macros[rec_idx].action = captured;
                    self.recording_macro = None;
                }
            }
        }

        let mut remove_idx: Option<usize> = None;
        for (idx, rule) in self.cfg.macros.iter_mut().enumerate() {
            egui::Frame::new()
                .stroke(Stroke::new(1.0, t.rule))
                .fill(t.paper)
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let state_label = if rule.enabled { "ON" } else { "OFF" };
                        if ink_button(ui, state_label, rule.enabled).clicked() {
                            rule.enabled = !rule.enabled;
                        }
                        let current_label = GESTURE_NAMES
                            .iter()
                            .find(|(id, _)| *id == rule.gesture)
                            .map(|(_, label)| *label)
                            .unwrap_or(rule.gesture.as_str());
                        egui::ComboBox::from_id_salt(("macro-gesture", idx))
                            .width(210.0)
                            .selected_text(current_label)
                            .show_ui(ui, |ui| {
                                for &(id, label) in GESTURE_NAMES {
                                    if ui.selectable_label(rule.gesture == id, label).clicked() {
                                        rule.gesture = id.to_string();
                                    }
                                }
                            });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ink_button(ui, "REMOVE", false).clicked() {
                                remove_idx = Some(idx);
                            }
                        });
                    });
                    ui.add_space(6.0);
                    egui::Grid::new(("macro-grid", idx))
                        .num_columns(2)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("APPS").size(11.0).color(t.ink_soft));
                            let mut apps_text = rule.apps.join(", ");
                            let edit = egui::TextEdit::singleline(&mut apps_text)
                                .desired_width(ui.available_width() - 10.0)
                                .hint_text("all apps, or e.g. firefox, code");
                            if ui.add(edit).changed() {
                                rule.apps = apps_text
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                            }
                            ui.end_row();

                            ui.label(egui::RichText::new("ACTION").size(11.0).color(t.ink_soft));
                            ui.horizontal(|ui| {
                                let recording = self.recording_macro == Some(idx);
                                let rec_label = if recording { "PRESS KEYS..." } else { "REC" };
                                let w = (ui.available_width() - 110.0).max(140.0);
                                ui.add(
                                    egui::TextEdit::singleline(&mut rule.action)
                                        .desired_width(w)
                                        .hint_text("e.g. ctrl+tab or command:..."),
                                );
                                if ink_button(ui, rec_label, recording)
                                    .on_hover_text("Click and press a key combo (Esc cancels)")
                                    .clicked()
                                {
                                    self.recording_macro = if recording { None } else { Some(idx) };
                                }
                            });
                            ui.end_row();
                        });

                    let act = rule.action.trim();
                    if !act.is_empty()
                        && !act.starts_with("command:")
                        && !act.starts_with("cmd:")
                        && !act.starts_with("shortcut:")
                        && !act.starts_with("dbus:")
                        && let Err(err) = parse_sequence(act.strip_prefix("key:").unwrap_or(act))
                    {
                        ui.add_space(4.0);
                        egui::Frame::new()
                            .fill(t.ink)
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(format!("Invalid macro: {err}"))
                                        .size(11.0)
                                        .color(t.paper),
                                );
                            });
                    }
                });
            ui.add_space(6.0);
        }

        if let Some(idx) = remove_idx {
            self.cfg.macros.remove(idx);
            if self.recording_macro == Some(idx) {
                self.recording_macro = None;
            }
        }

        if ink_button(ui, "+ ADD MACRO", false).clicked() {
            self.cfg.macros.push(MacroRule {
                enabled: true,
                name: String::new(),
                apps: Vec::new(),
                gesture: "swipe3_right".into(),
                action: String::new(),
            });
        }
    }

    fn look(&mut self, ui: &mut Ui) {
        section(ui, "06", "Look");
        ui.horizontal_wrapped(|ui| {
            for (mode, label, hint) in [
                (
                    ThemeMode::System,
                    "SYSTEM",
                    "Follow the system light/dark setting",
                ),
                (ThemeMode::Light, "LIGHT", "Black ink on grey paper"),
                (ThemeMode::Dark, "DARK", "The same print in negative"),
                (
                    ThemeMode::Sumi,
                    "SUMI",
                    "Washi paper, sumi ink and a vermilion seal",
                ),
            ] {
                let on = self.cfg.theme == mode;
                let resp = ink_button(ui, label, on).on_hover_text(hint);
                resp.widget_info(|| WidgetInfo::selected(Role::RadioButton, true, on, label));
                if resp.clicked() && !on {
                    self.cfg.theme = mode;
                    apply_theme(ui.ctx(), mode);
                }
            }
        });
        ui.add_space(6.0);
        let label = if self.cfg.ink_page {
            "INK PAGE  ON"
        } else {
            "INK PAGE  OFF"
        };
        let resp = ink_button(ui, label, self.cfg.ink_page)
            .on_hover_text("Drips, halftone and hatching behind the settings");
        resp.widget_info(|| {
            WidgetInfo::selected(Role::CheckBox, true, self.cfg.ink_page, "Ink page")
        });
        if resp.clicked() {
            self.cfg.ink_page = !self.cfg.ink_page;
        }
    }

    fn advanced(&mut self, ui: &mut Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new("05 / ADVANCED").font(FontId::new(12.0, strong())),
        )
        .show(ui, |ui| {
            egui::Grid::new("adv")
                .num_columns(2)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Dead zone before sliding").on_hover_text(
                        "Raw touchpad units the fingers must move before the value changes. \
                     Raise it if values change when you only meant to rest your fingers.",
                    );
                    ui.add(
                        egui::Slider::new(&mut self.cfg.edge_trigger_threshold, 0.0..=200.0)
                            .suffix(" units"),
                    );
                    ui.end_row();
                    ui.label("Second-finger wait").on_hover_text(
                        "How long a touch that lands in an edge zone waits for a second finger \
                     before it becomes normal pointer movement.",
                    );
                    ui.add(
                        egui::Slider::new(&mut self.cfg.settle_window_ms, 0..=120).suffix(" ms"),
                    );
                    ui.end_row();
                });
        });
    }

    fn footer(&mut self, ui: &mut Ui) {
        let t = Tones::ui(ui);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ink_button(ui, "UPDATE", false)
                    .on_hover_text("Download and install the latest release in a terminal window")
                    .clicked()
                {
                    self.notice = Some(match launch_updater() {
                        Ok(()) => Notice::Info(
                            "Updater opened. Reopen TouchpadFlow when it finishes.".into(),
                        ),
                        Err(e) => Notice::Error(format!("Could not start the updater: {e}")),
                    });
                }
                if ink_button(ui, "RESET DEFAULTS", false)
                    .on_hover_text("Left = brightness, right = volume, 22% zones")
                    .clicked()
                {
                    self.cfg = AppConfig::default();
                    apply_theme(ui.ctx(), self.cfg.theme);
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    match &self.notice {
                        // Errors are inverted (paper on ink) instead of coloured.
                        Some(Notice::Error(msg)) => {
                            egui::Frame::new()
                                .fill(t.ink)
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(msg).color(t.paper).size(12.0),
                                        )
                                        .wrap(),
                                    );
                                });
                        }
                        Some(Notice::Info(msg)) => {
                            ui.add(egui::Label::new(egui::RichText::new(msg).size(12.0)).wrap());
                        }
                        None => {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(self.path.display().to_string())
                                        .size(11.0)
                                        .color(t.ink),
                                )
                                .truncate(),
                            );
                        }
                    }
                });
            });
        });
    }
}

/// "02 / LEFT EDGE" followed by a drafting hairline.
fn section(ui: &mut Ui, num: &str, title: &str) {
    let t = Tones::ui(ui);
    ui.horizontal(|ui| {
        tag(
            ui,
            &format!("{num} / {}", title.to_uppercase()),
            FontId::new(12.0, strong()),
            t.accent,
        );
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 12.0), Sense::hover());
        ui.painter()
            .hline(r.x_range(), r.center().y, Stroke::new(2.0, t.ink));
    });
    ui.add_space(4.0);
}

/// Square button with a 2px ink border and a hard offset shadow; pressing sinks it.
fn ink_button(ui: &mut Ui, text: &str, inverted: bool) -> Response {
    let t = Tones::ui(ui);
    let font = FontId::new(13.0, strong());
    let probe = ui
        .painter()
        .layout_no_wrap(text.into(), font.clone(), t.ink);
    let body_size = probe.size() + Vec2::new(26.0, 14.0);
    let (rect, resp) = ui.allocate_exact_size(body_size + Vec2::splat(OFFSET), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let pressed = resp.is_pointer_button_down_on();
    let mut body = Rect::from_min_size(rect.min, body_size);
    let p = ui.painter();
    if pressed {
        body = body.translate(Vec2::splat(OFFSET - 1.0));
    } else {
        p.rect_filled(body.translate(Vec2::splat(OFFSET)), 0, t.ink);
    }
    let hovered = resp.hovered() && ui.is_enabled();
    let (fill, fg) = match (inverted, hovered) {
        (true, _) => (t.accent, t.paper),
        (false, true) => (t.rule, t.ink),
        (false, false) => (t.paper, t.ink),
    };
    p.rect_filled(body, 0, fill);
    p.rect_stroke(body, 0, Stroke::new(BORDER, t.ink), StrokeKind::Inside);
    let galley = p.layout_no_wrap(text.into(), font, fg);
    p.galley(body.center() - galley.size() / 2.0, galley, fg);
    if resp.has_focus() {
        p.rect_stroke(
            body.expand(3.0),
            0,
            Stroke::new(BORDER, t.ink),
            StrokeKind::Outside,
        );
    }
    resp
}

#[derive(Clone, Copy)]
enum Glyph {
    Solid,
    Hollow,
    Cross,
}

impl Glyph {
    fn paint(self, p: &Painter, r: Rect, ink: Color32) {
        match self {
            Glyph::Solid => {
                p.rect_filled(r, 0, ink);
            }
            Glyph::Hollow => {
                p.rect_stroke(r, 0, Stroke::new(BORDER, ink), StrokeKind::Inside);
            }
            Glyph::Cross => {
                let s = Stroke::new(BORDER, ink);
                p.line_segment([r.left_top(), r.right_bottom()], s);
                p.line_segment([r.right_top(), r.left_bottom()], s);
            }
        }
    }
}

fn zone_controls(
    ui: &mut Ui,
    id: &str,
    action: &mut EdgeAction,
    ratio: &mut f32,
    sensitivity: &mut f64,
) {
    let t = Tones::ui(ui);
    let caption = |ui: &mut Ui, s: &str| {
        ui.label(egui::RichText::new(s).size(11.0).color(t.ink_soft));
    };
    caption(ui, "CONTROLS");
    egui::ComboBox::from_id_salt(id)
        .width(ui.available_width() - OFFSET - 2.0)
        .selected_text(action_label(*action))
        .show_ui(ui, |ui| {
            for a in [EdgeAction::Brightness, EdgeAction::Volume, EdgeAction::None] {
                ui.selectable_value(action, a, action_label(a));
            }
        });
    let active = *action != EdgeAction::None;
    // Leave room for the value box so it is never clipped in a half-width column.
    ui.spacing_mut().slider_width = (ui.available_width() - 120.0).max(80.0);
    ui.add_space(6.0);
    caption(ui, "ZONE WIDTH");
    let (lo, hi) = EDGE_RATIO_RANGE;
    ui.add_enabled(
        active,
        egui::Slider::new(ratio, lo..=hi)
            .custom_formatter(|v, _| format!("{:.0}% of pad", v * 100.0))
            .custom_parser(parse_percent),
    );
    ui.add_space(6.0);
    caption(ui, "SENSITIVITY");
    let (lo, hi) = SENSITIVITY_RANGE;
    ui.add_enabled(
        active,
        egui::Slider::new(sensitivity, lo..=hi)
            .logarithmic(true)
            .custom_formatter(|v, _| format!("{:.0}% / slide", v * 100.0))
            .custom_parser(parse_percent),
    )
    .on_hover_text("How much one full top-to-bottom slide changes the value");
}

fn parse_percent(s: &str) -> Option<f64> {
    let n: f64 = s
        .trim()
        .trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.')
        .parse()
        .ok()?;
    Some(n / 100.0)
}

fn action_label(a: EdgeAction) -> &'static str {
    match a {
        EdgeAction::Brightness => "Brightness",
        EdgeAction::Volume => "Volume",
        EdgeAction::None => "Nothing (zone off)",
    }
}

/// 45° halftone screentone: offset rows of dots, clipped to `rect`.
fn screentone(p: &Painter, rect: Rect, pitch: f32, radius: f32, color: Color32) {
    let p = p.with_clip_rect(rect.intersect(p.clip_rect()));
    let mut row = 0;
    let mut y = rect.top() + pitch / 2.0;
    while y < rect.bottom() {
        let shift = if row % 2 == 0 { 0.0 } else { pitch / 2.0 };
        let mut x = rect.left() + pitch / 2.0 + shift;
        while x < rect.right() {
            p.circle_filled(Pos2::new(x, y), radius, color);
            x += pitch;
        }
        y += pitch * 0.5;
        row += 1;
    }
}

/// Signature element: a to-scale touchpad you resize by dragging the zone edges.
fn touchpad_map(ui: &mut Ui, cfg: &mut AppConfig) {
    let t = Tones::ui(ui);
    let width = ui.available_width().min(600.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, width * 0.46), Sense::hover());
    let pad = rect.shrink(12.0);
    let painter = ui.painter_at(rect);

    let (lo, hi) = EDGE_RATIO_RANGE;
    let zones = [
        (true, cfg.left_action, &mut cfg.left_edge_ratio),
        (false, cfg.right_action, &mut cfg.right_edge_ratio),
    ];
    for (is_left, action, ratio) in zones {
        let zone_w = pad.width() * *ratio;
        let zone = if is_left {
            Rect::from_min_size(pad.min, Vec2::new(zone_w, pad.height()))
        } else {
            Rect::from_min_size(
                Pos2::new(pad.max.x - zone_w, pad.min.y),
                Vec2::new(zone_w, pad.height()),
            )
        };
        let on = action != EdgeAction::None && cfg.enabled;
        // Clean paper under the zone so its tone does not moire with the page halftone.
        painter.rect_filled(zone, 0, t.paper);
        if on {
            screentone(&painter, zone, 7.0, 1.25, t.ink_soft);
        } else {
            screentone(&painter, zone, 11.0, 0.8, t.rule);
        }

        // Label plate in the middle of the zone.
        let name = match action {
            EdgeAction::Brightness => "BRIGHTNESS",
            EdgeAction::Volume => "VOLUME",
            EdgeAction::None => "OFF",
        };
        let label = painter.layout_no_wrap(
            name.into(),
            FontId::new(12.0, strong()),
            Color32::PLACEHOLDER,
        ); // colour picked below
        let pct = painter.layout_no_wrap(
            format!("{:.0}%", *ratio * 100.0),
            FontId::new(12.0, FontFamily::Monospace),
            t.ink,
        );
        let plate = Rect::from_center_size(
            zone.center() - Vec2::new(0.0, 10.0),
            label.size() + Vec2::new(12.0, 6.0),
        );
        let (plate_fill, plate_text) = if on {
            (t.ink, t.paper)
        } else {
            (t.paper, t.ink_soft)
        };
        painter.rect_filled(plate, 0, plate_fill);
        if !on {
            painter.rect_stroke(plate, 0, Stroke::new(1.0, t.rule), StrokeKind::Inside);
        }
        painter.galley(plate.min + Vec2::new(6.0, 3.0), label, plate_text);
        let pct_rect = Rect::from_center_size(
            plate.center_bottom() + Vec2::new(0.0, 14.0),
            pct.size() + Vec2::new(8.0, 2.0),
        );
        painter.rect_filled(pct_rect, 0, t.paper);
        painter.galley(pct_rect.min + Vec2::new(4.0, 1.0), pct, t.ink);

        if on {
            let x = zone.center().x;
            let arrow = Stroke::new(BORDER, t.ink);
            painter.arrow(
                Pos2::new(x, plate.top() - 14.0),
                Vec2::new(0.0, -30.0),
                arrow,
            );
            painter.arrow(
                Pos2::new(x, pct_rect.bottom() + 14.0),
                Vec2::new(0.0, 30.0),
                arrow,
            );
        }

        // Draggable boundary between this zone and the pointer area.
        let edge_x = if is_left { zone.max.x } else { zone.min.x };
        let handle = Rect::from_center_size(
            Pos2::new(edge_x, pad.center().y),
            Vec2::new(20.0, pad.height()),
        );
        let id = ui.scope_id().with(("zone-edge", is_left));
        let resp = ui
            .interact(handle, id, Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let frac = (p.x - pad.min.x) / pad.width();
            *ratio = (if is_left { frac } else { 1.0 - frac }).clamp(lo, hi);
        }
        let hot = resp.hovered() || resp.dragged();
        painter.vline(
            edge_x,
            pad.y_range(),
            Stroke::new(if hot { 4.5 } else { 3.0 }, t.ink),
        );
        // Grip: a small ink block on the line.
        let grip = Rect::from_center_size(Pos2::new(edge_x, pad.center().y), Vec2::new(10.0, 28.0));
        painter.rect_filled(grip, 0, t.ink);
        painter.hline(
            grip.x_range().shrink(2.5),
            grip.center().y - 4.0,
            Stroke::new(1.0, t.paper),
        );
        painter.hline(
            grip.x_range().shrink(2.5),
            grip.center().y + 4.0,
            Stroke::new(1.0, t.paper),
        );
    }

    // Pointer area: reticle + label.
    let mid = Pos2::new(
        pad.left()
            + pad.width() * cfg.left_edge_ratio
            + (pad.width() * (1.0 - cfg.left_edge_ratio - cfg.right_edge_ratio)) / 2.0,
        pad.center().y,
    );
    let rs = Stroke::new(2.0, t.ink_soft);
    painter.hline((mid.x - 8.0)..=(mid.x + 8.0), mid.y, rs);
    painter.vline(mid.x, (mid.y - 8.0)..=(mid.y + 8.0), rs);
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let c = mid + Vec2::new(sx * 36.0, sy * 36.0);
        painter.hline(c.x.min(c.x - sx * 10.0)..=c.x.max(c.x - sx * 10.0), c.y, rs);
        painter.vline(c.x, c.y.min(c.y - sy * 10.0)..=c.y.max(c.y - sy * 10.0), rs);
    }
    painter.text(
        mid + Vec2::new(0.0, 52.0),
        Align2::CENTER_CENTER,
        "POINTER",
        FontId::new(11.0, strong()),
        t.ink_soft,
    );
    painter.rect_stroke(pad, 0, Stroke::new(BORDER, t.ink), StrokeKind::Inside);

    ui.label(
        egui::RichText::new(
            "Drag a line to resize a zone. Slide two fingers up or down inside it.",
        )
        .italics()
        .size(11.5)
        .color(t.ink),
    );
}

fn capture_key_combo(ui: &Ui) -> Option<String> {
    let mut captured = None;
    ui.input(|i| {
        for ev in &i.events {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = ev
                && let Some(key_name) = egui_key_name(*key)
            {
                let mut parts = Vec::new();
                if modifiers.ctrl {
                    parts.push("ctrl");
                }
                if modifiers.shift {
                    parts.push("shift");
                }
                if modifiers.alt {
                    parts.push("alt");
                }
                if modifiers.mac_cmd || modifiers.command && !modifiers.ctrl {
                    parts.push("super");
                }
                parts.push(key_name);
                captured = Some(parts.join("+"));
                break;
            }
        }
    });
    captured
}

fn egui_key_name(key: egui::Key) -> Option<&'static str> {
    use egui::Key;
    Some(match key {
        Key::A => "a",
        Key::B => "b",
        Key::C => "c",
        Key::D => "d",
        Key::E => "e",
        Key::F => "f",
        Key::G => "g",
        Key::H => "h",
        Key::I => "i",
        Key::J => "j",
        Key::K => "k",
        Key::L => "l",
        Key::M => "m",
        Key::N => "n",
        Key::O => "o",
        Key::P => "p",
        Key::Q => "q",
        Key::R => "r",
        Key::S => "s",
        Key::T => "t",
        Key::U => "u",
        Key::V => "v",
        Key::W => "w",
        Key::X => "x",
        Key::Y => "y",
        Key::Z => "z",
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::F1 => "f1",
        Key::F2 => "f2",
        Key::F3 => "f3",
        Key::F4 => "f4",
        Key::F5 => "f5",
        Key::F6 => "f6",
        Key::F7 => "f7",
        Key::F8 => "f8",
        Key::F9 => "f9",
        Key::F10 => "f10",
        Key::F11 => "f11",
        Key::F12 => "f12",
        Key::Tab => "tab",
        Key::Enter => "enter",
        Key::Escape => "esc",
        Key::Space => "space",
        Key::Backspace => "backspace",
        Key::Delete => "delete",
        Key::Insert => "insert",
        Key::ArrowUp => "up",
        Key::ArrowDown => "down",
        Key::ArrowLeft => "left",
        Key::ArrowRight => "right",
        Key::Home => "home",
        Key::End => "end",
        Key::PageUp => "pageup",
        Key::PageDown => "pagedown",
        Key::Minus => "minus",
        Key::Equals => "equal",
        Key::OpenBracket => "leftbrace",
        Key::CloseBracket => "rightbrace",
        Key::Semicolon => "semicolon",
        Key::Quote => "apostrophe",
        Key::Backtick => "grave",
        Key::Backslash => "backslash",
        Key::Comma => "comma",
        Key::Period => "dot",
        Key::Slash => "slash",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_page_is_dark_at_the_frame_and_light_in_the_middle() {
        let (w, h) = (660.0, 900.0);
        let mean = |pts: &mut dyn Iterator<Item = (f32, f32)>| {
            let v: Vec<f32> = pts.map(|(x, y)| ink_at(x, y, w, h)).collect();
            v.iter().sum::<f32>() / v.len() as f32
        };
        let frame = mean(&mut (0..200).map(|i| (1.0, i as f32 * 4.5)));
        let middle = mean(&mut (0..200).map(|i| (330.0, 300.0 + i as f32)));
        assert!(frame > 0.9, "frame {frame}");
        assert!(middle < 0.35, "middle {middle}");
        let img = ink_page([66, 90], 1.0, Tones::of(false));
        assert_eq!(img.size, [66, 90]);
    }

    #[test]
    fn percent_parser_accepts_typed_values() {
        assert_eq!(parse_percent("30%"), Some(0.3));
        assert_eq!(parse_percent(" 150% / slide"), Some(1.5));
        assert_eq!(parse_percent("150"), Some(1.5));
        assert_eq!(parse_percent("abc"), None);
    }

    #[test]
    fn theme_mode_maps_to_egui_preference() {
        assert_eq!(
            to_egui_theme(ThemeMode::System),
            egui::ThemePreference::System
        );
        assert_eq!(
            to_egui_theme(ThemeMode::Light),
            egui::ThemePreference::Light
        );
        assert_eq!(to_egui_theme(ThemeMode::Dark), egui::ThemePreference::Dark);
        assert_eq!(to_egui_theme(ThemeMode::Sumi), egui::ThemePreference::Light);
    }
}
