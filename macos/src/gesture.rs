use crate::config::{EdgeAction, SharedConfig};
use crate::controls::{MacAudio, MacBrightness};
use crate::multitouch::MTContact;

pub struct MacGestureEngine {
    pub config: SharedConfig,
    active_side: i32, // -1: left zone, 1: right zone, 0: none
    start_y: f32,
    previous_y: f32,
    current_volume: f32,
}

impl MacGestureEngine {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            config,
            active_side: 0,
            start_y: 0.0,
            previous_y: 0.0,
            current_volume: MacAudio::get_volume(),
        }
    }

    pub fn handle_contacts(&mut self, contacts: &[MTContact]) {
        // Look for touching or moving fingers (state 2 or 3)
        let active: Vec<&MTContact> = contacts
            .iter()
            .filter(|c| c.state == 2 || c.state == 3)
            .collect();

        // Two-finger mode default
        if !self.config.enabled || active.len() != 2 {
            if self.active_side != 0 && active.is_empty() {
                self.active_side = 0;
            }
            return;
        }

        let avg_x = (active[0].normalized_position.x + active[1].normalized_position.x) / 2.0;
        let avg_y = (active[0].normalized_position.y + active[1].normalized_position.y) / 2.0;

        let c = &self.config;
        if self.active_side == 0 {
            if avg_x <= c.left_edge_ratio && c.left_action != EdgeAction::None {
                self.active_side = -1;
            } else if avg_x >= (1.0 - c.right_edge_ratio) && c.right_action != EdgeAction::None {
                self.active_side = 1;
            } else {
                return;
            }
            self.start_y = avg_y;
            self.previous_y = avg_y;
            self.current_volume = MacAudio::get_volume();
            return;
        }

        let dy = avg_y - self.previous_y;
        if dy.abs() < 0.005 {
            return;
        }

        let (action, sensitivity) = if self.active_side < 0 {
            (c.left_action, c.left_sensitivity)
        } else {
            (c.right_action, c.right_sensitivity)
        };
        match action {
            EdgeAction::Volume => {
                self.current_volume = (self.current_volume + dy * sensitivity).clamp(0.0, 1.0);
                MacAudio::set_volume(self.current_volume);
            }
            EdgeAction::Brightness => MacBrightness::set_brightness(dy),
            EdgeAction::None => {}
        }

        self.previous_y = avg_y;
    }
}
