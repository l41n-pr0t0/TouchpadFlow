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
    AppConfig, EDGE_RATIO_RANGE, EdgeAction, SENSITIVITY_RANGE, ThemeMode, config_path,
};

const AUTOSAVE_DELAY: Duration = Duration::from_millis(350);
const BORDER: f32 = 2.0;
const OFFSET: f32 = 3.0; // hard "press" shadow under buttons

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TouchpadFlow")
            .with_app_id("touchpadflow")
            .with_inner_size([660.0, 900.0])
            .with_min_inner_size([480.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "TouchpadFlow",
        options,
        Box::new(|cc| Ok(Box::new(SettingsApp::new(&cc.egui_ctx)))),
    )
}

// ---------------------------------------------------------------- theme

/// Exactly four tones, like a 2-bit print. Dark mode swaps them end for end.
#[derive(Clone, Copy)]
struct Tones {
    ink: Color32,
    ink_soft: Color32,
    rule: Color32,
    paper: Color32,
}

const C0: Color32 = Color32::from_rgb(0x0c, 0x0c, 0x0b);
const C1: Color32 = Color32::from_rgb(0x4a, 0x4a, 0x48);
const C2: Color32 = Color32::from_rgb(0x9b, 0x9b, 0x98);
const C3: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe2);

impl Tones {
    fn of(dark: bool) -> Self {
        if dark {
            Self {
                ink: C3,
                ink_soft: C2,
                rule: C1,
                paper: C0,
            }
        } else {
            Self {
                ink: C0,
                ink_soft: C1,
                rule: C2,
                paper: C3,
            }
        }
    }
    fn ui(ui: &Ui) -> Self {
        Self::of(ui.visuals().dark_mode)
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

fn visuals(dark: bool) -> Visuals {
    let t = Tones::of(dark);
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
    v.selection.bg_fill = t.ink;
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
        ThemeMode::Light => egui::ThemePreference::Light,
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
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.set_visuals_of(theme, visuals(theme == egui::Theme::Dark));
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

// ---------------------------------------------------------------- ui

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save_now();
        }
        let before = self.cfg.clone();
        let margin = egui::Frame::new()
            .fill(Tones::ui(ui).paper)
            .inner_margin(egui::Margin::symmetric(22, 14));

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
                        section(&mut cols[0], "02", "Left edge");
                        zone_controls(
                            &mut cols[0],
                            "left",
                            &mut self.cfg.left_action,
                            &mut self.cfg.left_edge_ratio,
                            &mut self.cfg.left_sensitivity,
                        );
                        section(&mut cols[1], "03", "Right edge");
                        zone_controls(
                            &mut cols[1],
                            "right",
                            &mut self.cfg.right_action,
                            &mut self.cfg.right_edge_ratio,
                            &mut self.cfg.right_sensitivity,
                        );
                    });
                    ui.add_space(14.0);
                    self.advanced(ui);
                });
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
            // Title plate: paper text on an ink block.
            let galley = ui.painter().layout_no_wrap(
                "TOUCHPADFLOW".into(),
                FontId::new(26.0, display()),
                t.paper,
            );
            let (rect, _) =
                ui.allocate_exact_size(galley.size() + Vec2::new(24.0, 10.0), Sense::hover());
            ui.painter().rect_filled(rect, 0, t.ink);
            ui.painter()
                .galley(rect.min + Vec2::new(12.0, 5.0), galley, t.paper);

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

                let dark = ui.visuals().dark_mode;
                let (theme_label, theme_hint) = if dark {
                    ("DARK", "Switch to light theme")
                } else {
                    ("LIGHT", "Switch to dark theme")
                };
                let theme_resp = ink_button(ui, theme_label, dark).on_hover_text(theme_hint);
                theme_resp
                    .widget_info(|| WidgetInfo::selected(Role::CheckBox, true, dark, "Dark theme"));
                if theme_resp.clicked() {
                    let next = if dark {
                        ThemeMode::Light
                    } else {
                        ThemeMode::Dark
                    };
                    self.cfg.theme = next;
                    ui.ctx().set_theme(to_egui_theme(next));
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

    fn advanced(&mut self, ui: &mut Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new("04 / ADVANCED").font(FontId::new(12.0, strong())),
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
                if ink_button(ui, "RESET DEFAULTS", false)
                    .on_hover_text("Left = brightness, right = volume, 22% zones")
                    .clicked()
                {
                    self.cfg = AppConfig::default();
                    ui.ctx().set_theme(to_egui_theme(self.cfg.theme));
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
                                        .color(t.ink_soft),
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
        ui.label(
            egui::RichText::new(format!("{num} / {}", title.to_uppercase()))
                .font(FontId::new(12.0, strong()))
                .color(t.ink),
        );
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 12.0), Sense::hover());
        ui.painter()
            .hline(r.x_range(), r.center().y, Stroke::new(1.0, t.rule));
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
        (true, _) => (t.ink, t.paper),
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

/// Drafting crosshair: a short "+" just outside a corner.
fn crosshair(p: &Painter, at: Pos2, color: Color32) {
    let s = Stroke::new(1.0, color);
    p.hline((at.x - 7.0)..=(at.x + 7.0), at.y, s);
    p.vline(at.x, (at.y - 7.0)..=(at.y + 7.0), s);
}

/// Signature element: a to-scale touchpad you resize by dragging the zone edges.
fn touchpad_map(ui: &mut Ui, cfg: &mut AppConfig) {
    let t = Tones::ui(ui);
    let width = ui.available_width().min(600.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, width * 0.46), Sense::hover());
    let pad = rect.shrink(12.0);
    let painter = ui.painter_at(rect);

    for c in [
        pad.left_top(),
        pad.right_top(),
        pad.left_bottom(),
        pad.right_bottom(),
    ] {
        let out = Vec2::new(
            if c.x < pad.center().x { -6.0 } else { 6.0 },
            if c.y < pad.center().y { -6.0 } else { 6.0 },
        );
        crosshair(&painter, c + out, t.rule);
    }
    painter.rect_filled(pad, 0, t.paper);

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
            Stroke::new(if hot { 4.0 } else { BORDER }, t.ink),
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
    let s = Stroke::new(1.0, t.rule);
    painter.circle_stroke(mid, 16.0, s);
    painter.hline((mid.x - 26.0)..=(mid.x + 26.0), mid.y, s);
    painter.vline(mid.x, (mid.y - 26.0)..=(mid.y + 26.0), s);
    painter.text(
        mid + Vec2::new(0.0, 40.0),
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
        .size(11.5)
        .color(t.ink_soft),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
