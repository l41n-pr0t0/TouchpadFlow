use std::collections::HashMap;
use std::time::{Duration, Instant};

use evdev::{AbsoluteAxisCode, EventType, InputEvent, KeyCode, SynchronizationCode};

use crate::action::{execute_action, BrightnessController, VolumeController};
use crate::config::AppConfig;
use crate::device::{TouchpadMetrics, VirtualOutput};
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Settling,
    Pointer,
    EdgeVolume,
    EdgeBrightness,
    Gesture3,
    Gesture4,
}

#[derive(Debug, Clone, Default)]
pub struct SlotData {
    pub active: bool,
    pub tracking_id: i32,
    pub start_x: Option<f64>,
    pub start_y: Option<f64>,
    pub current_x: Option<f64>,
    pub current_y: Option<f64>,
}

pub struct GestureEngine {
    config: AppConfig,
    metrics: TouchpadMetrics,
    slots: HashMap<usize, SlotData>,
    current_slot: usize,
    prev_finger_count: usize,

    session_mode: SessionMode,
    session_start_time: Instant,
    settle_peak: usize,
    buffer: Vec<InputEvent>,

    tap_eligible: bool,
    discrete_fired: bool,
    last_avg_x: Option<f64>,
    last_avg_y: Option<f64>,
    start_avg_x: Option<f64>,
    start_avg_y: Option<f64>,

    volume: VolumeController,
    brightness: BrightnessController,
    output: VirtualOutput,
}

impl GestureEngine {
    #[must_use]
    pub fn new(
        config: AppConfig,
        metrics: TouchpadMetrics,
        volume: VolumeController,
        brightness: BrightnessController,
        output: VirtualOutput,
    ) -> Self {
        Self {
            config,
            metrics,
            slots: HashMap::with_capacity(10),
            current_slot: 0,
            prev_finger_count: 0,

            session_mode: SessionMode::Pointer,
            session_start_time: Instant::now(),
            settle_peak: 0,
            buffer: Vec::with_capacity(256),

            tap_eligible: true,
            discrete_fired: false,
            last_avg_x: None,
            last_avg_y: None,
            start_avg_x: None,
            start_avg_y: None,

            volume,
            brightness,
            output,
        }
    }

    pub fn handle_event(&mut self, ev: InputEvent) -> Result<()> {
        if self.output.touchpad.is_some() {
            self.buffer.push(ev);
        }

        if ev.event_type() == EventType::ABSOLUTE {
            let code = AbsoluteAxisCode(ev.code());
            let val = ev.value();

            if code == AbsoluteAxisCode::ABS_MT_SLOT {
                self.current_slot = val.max(0) as usize;
            } else if code == AbsoluteAxisCode::ABS_MT_TRACKING_ID {
                let slot = self.slots.entry(self.current_slot).or_default();
                if val == -1 {
                    slot.active = false;
                } else {
                    slot.active = true;
                    slot.tracking_id = val;
                    slot.start_x = None;
                    slot.start_y = None;
                    slot.current_x = None;
                    slot.current_y = None;
                }
            } else if code == AbsoluteAxisCode::ABS_MT_POSITION_X {
                let slot = self.slots.entry(self.current_slot).or_default();
                if slot.start_x.is_none() {
                    slot.start_x = Some(val as f64);
                }
                slot.current_x = Some(val as f64);
            } else if code == AbsoluteAxisCode::ABS_MT_POSITION_Y {
                let slot = self.slots.entry(self.current_slot).or_default();
                if slot.start_y.is_none() {
                    slot.start_y = Some(val as f64);
                }
                slot.current_y = Some(val as f64);
            }
        } else if ev.event_type() == EventType::SYNCHRONIZATION
            && ev.code() == SynchronizationCode::SYN_REPORT.0
        {
            self.process_frame()?;
        }

        Ok(())
    }

    fn process_frame(&mut self) -> Result<()> {
        let active_slots: Vec<(usize, SlotData)> = self
            .slots
            .iter()
            .filter(|(_, s)| s.active)
            .map(|(&i, s)| (i, s.clone()))
            .collect();
        let finger_count = active_slots.len();

        if finger_count != self.prev_finger_count {
            self.handle_finger_transition(finger_count, &active_slots)?;
        }

        if self.session_mode == SessionMode::Settling {
            let elapsed = self.session_start_time.elapsed();
            if elapsed >= Duration::from_millis(self.config.settle_window_ms) {
                self.resolve_settle(&active_slots)?;
            }
        }

        if finger_count > 0 {
            self.track_motion(&active_slots)?;
        }

        self.handle_passthrough()?;
        self.prev_finger_count = finger_count;
        Ok(())
    }

    fn handle_finger_transition(
        &mut self,
        count: usize,
        active_slots: &[(usize, SlotData)],
    ) -> Result<()> {
        if self.prev_finger_count == 0 && count > 0 {
            self.session_start_time = Instant::now();
            self.settle_peak = count;
            self.tap_eligible = true;
            self.discrete_fired = false;
            self.last_avg_x = None;
            self.last_avg_y = None;
            self.start_avg_x = None;
            self.start_avg_y = None;

            if count == 2 {
                self.check_two_finger_edge(active_slots);
            } else if count >= 3 {
                self.session_mode = if count == 3 {
                    SessionMode::Gesture3
                } else {
                    SessionMode::Gesture4
                };
            } else {
                self.session_mode = SessionMode::Settling;
            }
        } else if count > self.settle_peak {
            self.settle_peak = count;
            if self.session_mode == SessionMode::Settling {
                if count == 2 {
                    self.check_two_finger_edge(active_slots);
                } else if count == 3 {
                    self.session_mode = SessionMode::Gesture3;
                } else if count >= 4 {
                    self.session_mode = SessionMode::Gesture4;
                }
            }
        }

        if count == 0 {
            self.on_session_end()?;
        }

        Ok(())
    }

    fn check_two_finger_edge(&mut self, active_slots: &[(usize, SlotData)]) {
        let valid_xs: Vec<f64> = active_slots.iter().filter_map(|(_, s)| s.current_x).collect();
        if valid_xs.len() == 2 {
            let left_r = self.config.left_edge_ratio;
            let right_r = self.config.right_edge_ratio;

            let both_left = valid_xs.iter().all(|&x| self.metrics.is_left_edge(x, left_r));
            let both_right = valid_xs.iter().all(|&x| self.metrics.is_right_edge(x, right_r));

            if both_left {
                self.session_mode = SessionMode::EdgeBrightness;
                self.buffer.clear();
                return;
            }

            if both_right {
                self.session_mode = SessionMode::EdgeVolume;
                self.buffer.clear();
                return;
            }
        }
        self.session_mode = SessionMode::Pointer;
    }

    fn resolve_settle(&mut self, active_slots: &[(usize, SlotData)]) -> Result<()> {
        if self.settle_peak >= 3 {
            self.session_mode = if self.settle_peak == 3 {
                SessionMode::Gesture3
            } else {
                SessionMode::Gesture4
            };
            let ids: Vec<usize> = active_slots.iter().map(|(i, _)| *i).collect();
            self.output.release_all_slots(&ids)?;
            self.buffer.clear();
        } else if self.settle_peak == 2 {
            self.check_two_finger_edge(active_slots);
            if self.session_mode != SessionMode::Pointer {
                let ids: Vec<usize> = active_slots.iter().map(|(i, _)| *i).collect();
                self.output.release_all_slots(&ids)?;
                self.buffer.clear();
            } else {
                self.flush_buffer()?;
            }
        } else {
            self.session_mode = SessionMode::Pointer;
            self.flush_buffer()?;
        }
        Ok(())
    }

    fn track_motion(&mut self, active_slots: &[(usize, SlotData)]) -> Result<()> {
        let coords: Vec<(f64, f64)> = active_slots
            .iter()
            .filter_map(|(_, s)| match (s.current_x, s.current_y) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            })
            .collect();

        if coords.is_empty() {
            return Ok(());
        }

        let avg_x: f64 = coords.iter().map(|(x, _)| *x).sum::<f64>() / coords.len() as f64;
        let avg_y: f64 = coords.iter().map(|(_, y)| *y).sum::<f64>() / coords.len() as f64;

        if self.start_avg_x.is_none() {
            self.start_avg_x = Some(avg_x);
            self.start_avg_y = Some(avg_y);
        }

        let start_x = self.start_avg_x.unwrap_or(avg_x);
        let start_y = self.start_avg_y.unwrap_or(avg_y);
        let total_dx = avg_x - start_x;
        let total_dy = avg_y - start_y;
        let dist = (total_dx * total_dx + total_dy * total_dy).sqrt();

        if dist > self.config.tap_threshold {
            self.tap_eligible = false;
        }

        let dy = match self.last_avg_y {
            Some(last_y) => avg_y - last_y,
            None => 0.0,
        };

        match self.session_mode {
            SessionMode::EdgeVolume => {
                let step = -dy * (self.config.volume_step / 10.0);
                self.volume.adjust(step);
            }
            SessionMode::EdgeBrightness => {
                let step = -dy * (self.config.brightness_step / 10.0);
                self.brightness.adjust(step);
            }
            SessionMode::Gesture3 => {
                if !self.discrete_fired && dist > self.config.discrete_threshold {
                    if total_dx.abs() > total_dy.abs() {
                        if total_dx > 0.0 {
                            self.fire_action("swipe3_right")?;
                        } else {
                            self.fire_action("swipe3_left")?;
                        }
                    } else if total_dy.abs() > total_dx.abs() {
                        let step = -dy * (self.config.volume_step / 10.0);
                        self.volume.adjust(step);
                    }
                    self.discrete_fired = true;
                }
            }
            SessionMode::Gesture4 => {
                if !self.discrete_fired && dist > self.config.discrete_threshold {
                    if total_dx.abs() > total_dy.abs() {
                        if total_dx > 0.0 {
                            self.fire_action("swipe4_right")?;
                        } else {
                            self.fire_action("swipe4_left")?;
                        }
                    } else if total_dy < 0.0 {
                        self.fire_action("swipe4_up")?;
                    } else {
                        self.fire_action("swipe4_down")?;
                    }
                    self.discrete_fired = true;
                }
            }
            _ => {}
        }

        self.last_avg_x = Some(avg_x);
        self.last_avg_y = Some(avg_y);
        Ok(())
    }

    fn on_session_end(&mut self) -> Result<()> {
        let dur = self.session_start_time.elapsed().as_secs_f64();

        if self.tap_eligible {
            if self.settle_peak == 3 && dur < self.config.tap_duration_limit_s {
                self.fire_action("three_finger_tap")?;
            } else if self.settle_peak == 4 && dur < self.config.four_finger_tap_duration_limit_s {
                self.fire_action("four_finger_tap")?;
            }
        }

        self.slots.clear();
        self.buffer.clear();
        self.session_mode = SessionMode::Pointer;
        self.settle_peak = 0;
        self.last_avg_x = None;
        self.last_avg_y = None;
        self.start_avg_x = None;
        self.start_avg_y = None;

        Ok(())
    }

    fn fire_action(&mut self, action_name: &str) -> Result<()> {
        if let Some(act) = self.config.shortcuts.get(action_name) {
            if let Some(key_name) = act.strip_prefix("key:") {
                let code = match key_name {
                    "BTN_MIDDLE" => KeyCode::BTN_MIDDLE,
                    "KEY_PLAYPAUSE" => KeyCode::KEY_PLAYPAUSE,
                    "KEY_NEXTSONG" => KeyCode::KEY_NEXTSONG,
                    "KEY_PREVIOUSSONG" => KeyCode::KEY_PREVIOUSSONG,
                    _ => KeyCode::BTN_MIDDLE,
                };
                self.output.tap_key(code)?;
            } else {
                execute_action(act);
            }
        }
        Ok(())
    }

    fn handle_passthrough(&mut self) -> Result<()> {
        match self.session_mode {
            SessionMode::Settling => {}
            SessionMode::Pointer => {
                self.flush_buffer()?;
            }
            SessionMode::EdgeVolume
            | SessionMode::EdgeBrightness
            | SessionMode::Gesture3
            | SessionMode::Gesture4 => {
                self.buffer.clear();
            }
        }
        Ok(())
    }

    fn flush_buffer(&mut self) -> Result<()> {
        if !self.buffer.is_empty() {
            let evs = std::mem::take(&mut self.buffer);
            self.output.forward_raw(&evs)?;
        }
        Ok(())
    }
}
