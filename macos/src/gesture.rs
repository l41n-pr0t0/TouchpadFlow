use crate::controls::{MacAudio, MacBrightness};
use crate::multitouch::MTContact;

pub struct MacGestureEngine {
    left_edge_ratio: f32,
    right_edge_ratio: f32,
    active_side: i32, // -1: left (brightness), 1: right (volume), 0: none
    start_y: f32,
    previous_y: f32,
    current_volume: f32,
}

impl MacGestureEngine {
    pub fn new(left_edge_ratio: f32, right_edge_ratio: f32) -> Self {
        Self {
            left_edge_ratio,
            right_edge_ratio,
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
        if active.len() != 2 {
            if self.active_side != 0 && active.is_empty() {
                self.active_side = 0;
            }
            return;
        }

        let avg_x = (active[0].normalized_position.x + active[1].normalized_position.x) / 2.0;
        let avg_y = (active[0].normalized_position.y + active[1].normalized_position.y) / 2.0;

        if self.active_side == 0 {
            if avg_x <= self.left_edge_ratio {
                self.active_side = -1; // Brightness
                self.start_y = avg_y;
                self.previous_y = avg_y;
            } else if avg_x >= (1.0 - self.right_edge_ratio) {
                self.active_side = 1; // Volume
                self.start_y = avg_y;
                self.previous_y = avg_y;
                self.current_volume = MacAudio::get_volume();
            }
            return;
        }

        let dy = avg_y - self.previous_y;
        if dy.abs() < 0.005 {
            return;
        }

        if self.active_side == 1 {
            // Volume
            self.current_volume = (self.current_volume + dy * 0.8).clamp(0.0, 1.0);
            MacAudio::set_volume(self.current_volume);
        } else if self.active_side == -1 {
            // Brightness
            MacBrightness::set_brightness(dy);
        }

        self.previous_y = avg_y;
    }
}
