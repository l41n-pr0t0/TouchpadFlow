use std::time::{Duration, Instant};

use evdev::{AbsoluteAxisCode, EventType, InputEvent, KeyCode, SynchronizationCode};

use crate::action::{BrightnessController, VolumeController, execute_action};
use crate::config::{AppConfig, EdgeAction};
use crate::device::{TouchpadMetrics, VirtualOutput};
use crate::error::Result;

pub const MAX_SLOTS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Settling,
    Pointer,
    EdgeVolume,
    EdgeBrightness,
    Gesture3,
    Gesture4,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SlotData {
    pub active: bool,
    pub current_x: Option<f64>,
    pub current_y: Option<f64>,
}

pub struct GestureEngine {
    config: AppConfig,
    metrics: TouchpadMetrics,
    slots: [SlotData; MAX_SLOTS],
    current_slot: usize,
    prev_finger_count: usize,

    session_mode: SessionMode,
    session_start_time: Instant,
    settle_peak: usize,
    buffer: Vec<InputEvent>,
    buffer_start_slot: usize,
    edge_sensitivity: f64,

    tap_eligible: bool,
    discrete_fired: bool,
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
            slots: [SlotData::default(); MAX_SLOTS],
            current_slot: 0,
            prev_finger_count: 0,

            session_mode: SessionMode::Pointer,
            session_start_time: Instant::now(),
            settle_peak: 0,
            buffer: Vec::with_capacity(256),
            buffer_start_slot: 0,
            edge_sensitivity: 0.0,

            tap_eligible: true,
            discrete_fired: false,
            last_avg_y: None,
            start_avg_x: None,
            start_avg_y: None,

            volume,
            brightness,
            output,
        }
    }

    /// Dynamic live reload of configuration
    pub fn update_config(&mut self, config: AppConfig) {
        self.config = config;
    }

    #[must_use]
    pub fn session_mode(&self) -> SessionMode {
        self.session_mode
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    /// Emergency release of all virtual slots and reset to transparent pass-through
    pub fn emergency_reset(&mut self) -> Result<()> {
        let mut active_ids = [0usize; MAX_SLOTS];
        let mut active_count = 0;
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.active {
                active_ids[active_count] = i;
                active_count += 1;
            }
        }
        if active_count > 0 {
            self.output.release_all_slots(&active_ids[..active_count])?;
        }
        for slot in &mut self.slots {
            slot.active = false;
        }
        self.buffer.clear();
        self.session_mode = SessionMode::Pointer;
        self.prev_finger_count = 0;
        self.settle_peak = 0;
        self.last_avg_y = None;
        self.start_avg_x = None;
        self.start_avg_y = None;
        Ok(())
    }

    pub fn handle_event(&mut self, ev: InputEvent) -> Result<()> {
        if self.output.touchpad.is_some() {
            if self.buffer.is_empty() {
                self.buffer_start_slot = self.current_slot;
            }
            self.buffer.push(ev);
        }

        if ev.event_type() == EventType::ABSOLUTE {
            let code = AbsoluteAxisCode(ev.code());
            let val = ev.value();

            if code == AbsoluteAxisCode::ABS_MT_SLOT {
                self.current_slot = val.max(0) as usize;
            } else if self.current_slot < MAX_SLOTS {
                let slot = &mut self.slots[self.current_slot];
                if code == AbsoluteAxisCode::ABS_MT_TRACKING_ID {
                    if val == -1 {
                        slot.active = false;
                    } else {
                        // Keep last position: the kernel omits X/Y that did not change.
                        slot.active = true;
                    }
                } else if code == AbsoluteAxisCode::ABS_MT_POSITION_X {
                    slot.current_x = Some(val as f64);
                } else if code == AbsoluteAxisCode::ABS_MT_POSITION_Y {
                    slot.current_y = Some(val as f64);
                }
            }
        } else if ev.event_type() == EventType::SYNCHRONIZATION
            && ev.code() == SynchronizationCode::SYN_REPORT.0
        {
            self.process_frame()?;
        }

        Ok(())
    }

    fn process_frame(&mut self) -> Result<()> {
        let mut active_indices = [0usize; MAX_SLOTS];
        let mut finger_count = 0;
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.active {
                active_indices[finger_count] = i;
                finger_count += 1;
            }
        }
        let active = &active_indices[..finger_count];

        if finger_count != self.prev_finger_count {
            self.handle_finger_transition(finger_count, active)?;
        }

        // A tap can be just a down and an up frame (idle pads send nothing in between),
        // so a session that ends while settling must still resolve and be delivered.
        if self.session_mode == SessionMode::Settling
            && (finger_count == 0
                || self.session_start_time.elapsed()
                    >= Duration::from_millis(self.config.settle_window_ms))
        {
            self.resolve_settle(active);
        }

        if finger_count > 0 {
            self.track_motion(active)?;
        }

        // Forward before ending the session so the lift frame reaches the virtual pad.
        self.handle_passthrough()?;

        if finger_count == 0 && self.prev_finger_count > 0 {
            self.on_session_end()?;
        }
        self.prev_finger_count = finger_count;
        Ok(())
    }

    fn handle_finger_transition(&mut self, count: usize, active_indices: &[usize]) -> Result<()> {
        if self.prev_finger_count == 0 && count > 0 {
            self.session_start_time = Instant::now();
            self.settle_peak = count;
            self.tap_eligible = true;
            self.discrete_fired = false;
            self.last_avg_y = None;
            self.start_avg_x = None;
            self.start_avg_y = None;

            match count {
                // Only touches that land in an edge zone wait for a possible second finger;
                // everything else goes to the pointer immediately (no added latency).
                1 if self.edge_zone_action(active_indices[0]).is_some() => {
                    self.session_mode = SessionMode::Settling;
                }
                1 => self.session_mode = SessionMode::Pointer,
                2 => self.check_two_finger_edge(active_indices),
                _ => self.enter_gesture(count, active_indices)?,
            }
        } else if count > self.settle_peak {
            self.settle_peak = count;
            match self.session_mode {
                SessionMode::Settling if count == 2 => self.check_two_finger_edge(active_indices),
                SessionMode::Settling | SessionMode::Pointer if count >= 3 => {
                    self.enter_gesture(count, active_indices)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Switch to a 3/4-finger gesture and lift any touches already sent to the virtual pad.
    fn enter_gesture(&mut self, count: usize, active_indices: &[usize]) -> Result<()> {
        if self.session_mode == SessionMode::Pointer {
            self.output.release_all_slots(active_indices)?;
        }
        self.session_mode = if count == 3 {
            SessionMode::Gesture3
        } else {
            SessionMode::Gesture4
        };
        Ok(())
    }

    /// Which edge action (if any) a single finger in `slot` is resting in.
    fn edge_zone_action(&self, slot: usize) -> Option<(SessionMode, f64)> {
        let x = self.slots[slot].current_x?;
        let c = &self.config;
        if self.metrics.is_left_edge(x, c.left_edge_ratio) {
            edge_mode(c.left_action, c.left_sensitivity)
        } else if self.metrics.is_right_edge(x, c.right_edge_ratio) {
            edge_mode(c.right_action, c.right_sensitivity)
        } else {
            None
        }
    }

    fn check_two_finger_edge(&mut self, active_indices: &[usize]) {
        self.session_mode = SessionMode::Pointer;
        let [i0, i1] = active_indices else { return };
        let (Some(x0), Some(x1)) = (self.slots[*i0].current_x, self.slots[*i1].current_x) else {
            return;
        };
        let c = &self.config;
        let avg_x = (x0 + x1) / 2.0;

        // Centroid edge detection with edge tolerance for natural finger width
        let in_zone = |ratio: f32, test: fn(&TouchpadMetrics, f64, f32) -> bool| {
            test(&self.metrics, avg_x, ratio)
                || test(&self.metrics, x0, ratio * 0.70)
                || test(&self.metrics, x1, ratio * 0.70)
        };

        let picked = if in_zone(c.left_edge_ratio, TouchpadMetrics::is_left_edge) {
            edge_mode(c.left_action, c.left_sensitivity)
        } else if in_zone(c.right_edge_ratio, TouchpadMetrics::is_right_edge) {
            edge_mode(c.right_action, c.right_sensitivity)
        } else {
            None
        };
        if let Some((mode, sensitivity)) = picked {
            self.session_mode = mode;
            self.edge_sensitivity = sensitivity;
        }
    }

    fn resolve_settle(&mut self, active_indices: &[usize]) {
        // Nothing was forwarded while settling, so there is nothing to release here.
        if self.settle_peak == 2 {
            self.check_two_finger_edge(active_indices);
        } else {
            self.session_mode = SessionMode::Pointer;
        }
    }

    fn track_motion(&mut self, active_indices: &[usize]) -> Result<()> {
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut count = 0;

        for &idx in active_indices {
            let slot = &self.slots[idx];
            if let (Some(x), Some(y)) = (slot.current_x, slot.current_y) {
                sum_x += x;
                sum_y += y;
                count += 1;
            }
        }

        if count == 0 {
            return Ok(());
        }

        let avg_x = sum_x / count as f64;
        let avg_y = sum_y / count as f64;

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
            SessionMode::EdgeVolume | SessionMode::EdgeBrightness
                if total_dy.abs() >= self.config.edge_trigger_threshold =>
            {
                // Normalised by pad height so sensitivity means the same on every touchpad.
                let step = -dy / self.metrics.height() * self.edge_sensitivity;
                if self.session_mode == SessionMode::EdgeVolume {
                    self.volume.adjust(step);
                } else {
                    self.brightness.adjust(step);
                }
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
                        let step = -dy / self.metrics.height() * self.config.right_sensitivity;
                        self.volume.adjust(step);
                    }
                    self.discrete_fired = true;
                }
            }
            SessionMode::Gesture4
                if !self.discrete_fired && dist > self.config.discrete_threshold =>
            {
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
            _ => {}
        }

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

        self.buffer.clear();
        self.session_mode = SessionMode::Pointer;
        self.settle_peak = 0;
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
            SessionMode::Pointer => self.flush_buffer()?,
            SessionMode::EdgeVolume
            | SessionMode::EdgeBrightness
            | SessionMode::Gesture3
            | SessionMode::Gesture4 => {
                // Drop touches/clicks but keep positions in sync, otherwise the next
                // pointer touch starts from a stale position and the cursor jumps.
                self.buffer.retain(|ev| {
                    ev.event_type() != EventType::KEY
                        && !(ev.event_type() == EventType::ABSOLUTE
                            && ev.code() == AbsoluteAxisCode::ABS_MT_TRACKING_ID.0)
                });
                self.flush_buffer()?;
            }
        }
        Ok(())
    }

    fn flush_buffer(&mut self) -> Result<()> {
        if !self.buffer.is_empty() {
            let evs = std::mem::take(&mut self.buffer);
            self.output.forward_raw(self.buffer_start_slot, &evs)?;
            self.buffer = evs;
            self.buffer.clear();
        }
        Ok(())
    }
}

fn edge_mode(action: EdgeAction, sensitivity: f64) -> Option<(SessionMode, f64)> {
    match action {
        EdgeAction::Volume => Some((SessionMode::EdgeVolume, sensitivity)),
        EdgeAction::Brightness => Some((SessionMode::EdgeBrightness, sensitivity)),
        EdgeAction::None => None,
    }
}
