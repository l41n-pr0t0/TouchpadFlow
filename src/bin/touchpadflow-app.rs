//! TouchpadFlow settings app: native window (egui), no browser, no network.
//! Edits the shared config file; every engine (Linux daemon, macOS, Windows) reads it.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use touchpadflow::config::{
    AppConfig, EDGE_RATIO_RANGE, EdgeAction, SENSITIVITY_RANGE, config_path,
};

// Palette (see DESIGN.md): zinc neutrals + one accent, "Trackpad Teal".
const ACCENT: Color32 = Color32::from_rgb(0x1F, 0x9E, 0x8A);
const DANGER: Color32 = Color32::from_rgb(0xC2, 0x4A, 0x3F);
const WARN: Color32 = Color32::from_rgb(0xB7, 0x86, 0x2C);
const AUTOSAVE_DELAY: Duration = Duration::from_millis(350);

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TouchpadFlow")
            .with_app_id("touchpadflow")
            .with_inner_size([640.0, 780.0])
            .with_min_inner_size([460.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "TouchpadFlow",
        options,
        Box::new(|cc| Ok(Box::new(SettingsApp::new(&cc.egui_ctx)))),
    )
}

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
        apply_theme(ctx);
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
            device: v["device"]
                .as_str()
                .filter(|d| !d.is_empty())
                .map(|d| format!("on {d}"))
                .unwrap_or_default(),
        };
    }
    Engine::NotRunning
}

fn apply_theme(ctx: &egui::Context) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            let v = &mut style.visuals;
            if theme == egui::Theme::Dark {
                v.panel_fill = Color32::from_rgb(0x18, 0x18, 0x1B); // zinc-900, never #000
                v.extreme_bg_color = Color32::from_rgb(0x09, 0x09, 0x0B);
                v.faint_bg_color = Color32::from_rgb(0x27, 0x27, 0x2A);
            } else {
                v.panel_fill = Color32::from_rgb(0xFA, 0xFA, 0xFA);
                v.faint_bg_color = Color32::from_rgb(0xF4, 0xF4, 0xF5);
            }
            v.selection.bg_fill = ACCENT;
            v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
            v.hyperlink_color = ACCENT;
            v.slider_trailing_fill = true;
            style.spacing.item_spacing = Vec2::new(10.0, 8.0);
            style.spacing.interact_size.y = 26.0;
        });
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save_now();
        }
        let before = self.cfg.clone();

        egui::Panel::bottom("footer").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                match &self.notice {
                    Some(Notice::Error(msg)) => {
                        ui.colored_label(DANGER, msg);
                    }
                    Some(Notice::Info(msg)) => {
                        ui.weak(msg);
                    }
                    None => {
                        ui.weak(format!("Config: {}", self.path.display()));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("Reset to defaults")
                        .on_hover_text("Left = brightness, right = volume, 22% zones")
                        .clicked()
                    {
                        self.cfg = AppConfig::default();
                    }
                });
            });
            ui.add_space(6.0);
        });

        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.header(ui);
                ui.add_space(12.0);
                ui.add_enabled_ui(self.cfg.enabled, |ui| {
                    touchpad_map(ui, &mut self.cfg);
                    ui.add_space(14.0);
                    ui.columns(2, |cols| {
                        zone_controls(
                            &mut cols[0],
                            "Left edge",
                            &mut self.cfg.left_action,
                            &mut self.cfg.left_edge_ratio,
                            &mut self.cfg.left_sensitivity,
                        );
                        zone_controls(
                            &mut cols[1],
                            "Right edge",
                            &mut self.cfg.right_action,
                            &mut self.cfg.right_edge_ratio,
                            &mut self.cfg.right_sensitivity,
                        );
                    });
                    ui.add_space(10.0);
                    egui::CollapsingHeader::new("Advanced").show(ui, |ui| {
                        egui::Grid::new("adv").num_columns(2).show(ui, |ui| {
                            ui.label("Dead zone before sliding")
                                .on_hover_text("Raw touchpad units the fingers must move before the value changes. Raise it if values change when you only meant to rest your fingers.");
                            ui.add(egui::Slider::new(&mut self.cfg.edge_trigger_threshold, 0.0..=200.0).suffix(" units"));
                            ui.end_row();
                            ui.label("Second-finger wait")
                                .on_hover_text("How long a touch that lands in an edge zone waits for a second finger before it becomes normal pointer movement.");
                            ui.add(egui::Slider::new(&mut self.cfg.settle_window_ms, 0..=120).suffix(" ms"));
                            ui.end_row();
                        });
                    });
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
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("TouchpadFlow").size(22.0).strong());
                let (color, text) = match self.engine() {
                    Engine::Checking => (
                        ui.visuals().weak_text_color(),
                        "Checking engine…".to_string(),
                    ),
                    Engine::Running { paused: true, .. } => (WARN, "Engine paused".into()),
                    Engine::Running { .. } if !self.cfg.enabled => {
                        (WARN, "Engine idle — gestures off".to_string())
                    }
                    Engine::Running { device, .. } => (
                        ACCENT,
                        format!("Engine running {device}").trim_end().to_string(),
                    ),
                    Engine::NotRunning => (DANGER, "Engine not running".into()),
                    Engine::WatchesFile => (
                        ui.visuals().weak_text_color(),
                        "Settings apply to the platform engine".into(),
                    ),
                };
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 4.0, color);
                    ui.colored_label(color, text);
                });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if self.cfg.enabled {
                    "Gestures on"
                } else {
                    "Gestures off"
                };
                ui.toggle_value(&mut self.cfg.enabled, label)
                    .on_hover_text("Off hands the touchpad straight back to the system");
            });
        });
    }
}

fn zone_controls(
    ui: &mut egui::Ui,
    title: &str,
    action: &mut EdgeAction,
    ratio: &mut f32,
    sensitivity: &mut f64,
) {
    ui.label(egui::RichText::new(title).strong());
    ui.add_space(2.0);
    ui.label("Controls");
    egui::ComboBox::from_id_salt(title)
        .selected_text(action_label(*action))
        .show_ui(ui, |ui| {
            for a in [EdgeAction::Brightness, EdgeAction::Volume, EdgeAction::None] {
                ui.selectable_value(action, a, action_label(a));
            }
        });
    let active = *action != EdgeAction::None;
    // Leave room for the value box so it is never clipped in a half-width column.
    ui.spacing_mut().slider_width = (ui.available_width() - 130.0).max(80.0);
    ui.add_space(4.0);
    ui.label("Zone width");
    let (lo, hi) = EDGE_RATIO_RANGE;
    ui.add_enabled(
        active,
        egui::Slider::new(ratio, lo..=hi)
            .custom_formatter(|v, _| format!("{:.0}% of pad", v * 100.0))
            .custom_parser(parse_percent),
    );
    ui.add_space(4.0);
    ui.label("Sensitivity")
        .on_hover_text("How much one full top-to-bottom slide changes the value");
    let (lo, hi) = SENSITIVITY_RANGE;
    ui.add_enabled(
        active,
        egui::Slider::new(sensitivity, lo..=hi)
            .logarithmic(true)
            .custom_formatter(|v, _| format!("{:.0}% per slide", v * 100.0))
            .custom_parser(parse_percent),
    );
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

/// Signature element: a to-scale touchpad you resize by dragging the zone edges.
fn touchpad_map(ui: &mut egui::Ui, cfg: &mut AppConfig) {
    let width = ui.available_width().min(560.0);
    let size = Vec2::new(width, width * 0.58);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let pad = rect.shrink(2.0);
    let visuals = ui.visuals().clone();
    let painter = ui.painter_at(rect);
    let radius = CornerRadius::same(14);

    painter.rect_filled(pad, radius, visuals.faint_bg_color);
    painter.rect_stroke(
        pad,
        radius,
        visuals.widgets.inactive.bg_stroke,
        StrokeKind::Inside,
    );

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
        let fill = if on {
            ACCENT.gamma_multiply(0.22)
        } else {
            visuals.extreme_bg_color.gamma_multiply(0.5)
        };
        let corner = if is_left {
            CornerRadius {
                nw: 14,
                sw: 14,
                ne: 0,
                se: 0,
            }
        } else {
            CornerRadius {
                nw: 0,
                sw: 0,
                ne: 14,
                se: 14,
            }
        };
        painter.rect_filled(zone, corner, fill);

        let text_color = if on {
            visuals.strong_text_color()
        } else {
            visuals.weak_text_color()
        };
        painter.text(
            zone.center() - Vec2::new(0.0, 12.0),
            Align2::CENTER_CENTER,
            action_label(action).split(' ').next().unwrap_or(""),
            FontId::proportional(14.0),
            text_color,
        );
        painter.text(
            zone.center() + Vec2::new(0.0, 10.0),
            Align2::CENTER_CENTER,
            format!("{:.0}%", *ratio * 100.0),
            FontId::monospace(12.0),
            visuals.weak_text_color(),
        );
        if on {
            let x = zone.center().x;
            let arrow = Stroke::new(1.5, ACCENT);
            painter.arrow(
                Pos2::new(x, zone.center().y - 34.0),
                Vec2::new(0.0, -26.0),
                arrow,
            );
            painter.arrow(
                Pos2::new(x, zone.center().y + 34.0),
                Vec2::new(0.0, 26.0),
                arrow,
            );
        }

        // Draggable boundary between this zone and the pointer area.
        let edge_x = if is_left { zone.max.x } else { zone.min.x };
        let handle = Rect::from_center_size(
            Pos2::new(edge_x, pad.center().y),
            Vec2::new(18.0, pad.height()),
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
        let line = if resp.hovered() || resp.dragged() {
            Stroke::new(3.0, ACCENT)
        } else {
            Stroke::new(1.5, visuals.widgets.inactive.fg_stroke.color)
        };
        painter.line_segment(
            [
                Pos2::new(edge_x, pad.min.y + 10.0),
                Pos2::new(edge_x, pad.max.y - 10.0),
            ],
            line,
        );
    }

    painter.text(
        pad.center(),
        Align2::CENTER_CENTER,
        "Pointer",
        FontId::proportional(13.0),
        visuals.weak_text_color(),
    );
    ui.weak("Drag the lines to resize a zone. Slide two fingers up or down inside it.");
}

#[cfg(test)]
mod tests {
    #[test]
    fn percent_parser_accepts_typed_values() {
        assert_eq!(super::parse_percent("30%"), Some(0.3));
        assert_eq!(super::parse_percent(" 150% per slide"), Some(1.5));
        assert_eq!(super::parse_percent("abc"), None);
        assert_eq!(super::parse_percent("150"), Some(1.5));
    }
}
