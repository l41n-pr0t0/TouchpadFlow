use std::fs;
use std::path::PathBuf;

use evdev::uinput::VirtualDevice;
use evdev::{
    AbsoluteAxisCode, AttributeSet, Device, EventType, InputEvent, KeyCode, PropType,
    SynchronizationCode,
};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy)]
pub struct TouchpadMetrics {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

impl TouchpadMetrics {
    #[must_use]
    pub fn from_device(dev: &Device) -> Self {
        let mut min_x = 0;
        let mut max_x = 1000;
        let mut min_y = 0;
        let mut max_y = 1000;

        if let Ok(abs) = dev.get_abs_state() {
            let x_info = abs[AbsoluteAxisCode::ABS_MT_POSITION_X.0 as usize];
            if x_info.maximum > x_info.minimum {
                min_x = x_info.minimum;
                max_x = x_info.maximum;
            }
            let y_info = abs[AbsoluteAxisCode::ABS_MT_POSITION_Y.0 as usize];
            if y_info.maximum > y_info.minimum {
                min_y = y_info.minimum;
                max_y = y_info.maximum;
            }
        }

        Self {
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }

    #[must_use]
    pub fn normalize_x(&self, raw: f64) -> f32 {
        let width = (self.max_x - self.min_x) as f64;
        if width <= 0.0 {
            return 0.5;
        }
        let clamped = raw.clamp(self.min_x as f64, self.max_x as f64);
        ((clamped - self.min_x as f64) / width) as f32
    }

    #[must_use]
    pub fn is_left_edge(&self, raw_x: f64, ratio: f32) -> bool {
        self.normalize_x(raw_x) <= ratio
    }

    #[must_use]
    pub fn is_right_edge(&self, raw_x: f64, ratio: f32) -> bool {
        self.normalize_x(raw_x) >= (1.0 - ratio)
    }
}

pub struct VirtualOutput {
    pub touchpad: Option<VirtualDevice>,
    pub keyboard: Option<VirtualDevice>,
}

impl VirtualOutput {
    pub fn new(phys_dev: &Device, grab_mode: bool) -> Result<Self> {
        let keyboard = Self::create_virtual_keyboard()?;

        let touchpad = if grab_mode {
            Some(Self::create_virtual_touchpad(phys_dev)?)
        } else {
            None
        };

        Ok(Self {
            touchpad,
            keyboard: Some(keyboard),
        })
    }

    #[must_use]
    pub fn dummy() -> Self {
        Self {
            touchpad: None,
            keyboard: None,
        }
    }

    fn create_virtual_keyboard() -> Result<VirtualDevice> {
        let mut keys: AttributeSet<KeyCode> = AttributeSet::new();
        keys.insert(KeyCode::BTN_LEFT);
        keys.insert(KeyCode::BTN_RIGHT);
        keys.insert(KeyCode::BTN_MIDDLE);
        keys.insert(KeyCode::KEY_PLAYPAUSE);
        keys.insert(KeyCode::KEY_NEXTSONG);
        keys.insert(KeyCode::KEY_PREVIOUSSONG);
        keys.insert(KeyCode::KEY_VOLUMEUP);
        keys.insert(KeyCode::KEY_VOLUMEDOWN);
        keys.insert(KeyCode::KEY_MUTE);
        keys.insert(KeyCode::KEY_BRIGHTNESSUP);
        keys.insert(KeyCode::KEY_BRIGHTNESSDOWN);

        let dev = VirtualDevice::builder()
            .map_err(|e| AppError::Uinput(format!("failed opening uinput: {e}")))?
            .name("TouchpadFlow Virtual Keyboard")
            .with_keys(&keys)
            .map_err(|e| AppError::Uinput(format!("failed setting keys: {e}")))?
            .build()
            .map_err(|e| AppError::Uinput(format!("failed building virtual keyboard: {e}")))?;

        Ok(dev)
    }

    fn create_virtual_touchpad(phys: &Device) -> Result<VirtualDevice> {
        let mut builder = VirtualDevice::builder()
            .map_err(|e| AppError::Uinput(format!("failed opening uinput: {e}")))?
            .name("TouchpadFlow Virtual Touchpad");

        if let Some(keys) = phys.supported_keys() {
            builder = builder
                .with_keys(keys)
                .map_err(|e| AppError::Uinput(format!("failed copying keys: {e}")))?;
        }

        if let Some(props) = Some(phys.properties()) {
            builder = builder
                .with_properties(props)
                .map_err(|e| AppError::Uinput(format!("failed copying properties: {e}")))?;
        }

        if let Ok(abs_state) = phys.get_abs_state() {
            if let Some(supported_abs) = phys.supported_absolute_axes() {
                for code in supported_abs.iter() {
                    let info = abs_state[code.0 as usize];
                    let setup = evdev::UinputAbsSetup::new(code, info.into());
                    builder = builder.with_absolute_axis(&setup).map_err(|e| {
                        AppError::Uinput(format!("failed configuring abs axis {code:?}: {e}"))
                    })?;
                }
            }
        }

        let dev = builder
            .build()
            .map_err(|e| AppError::Uinput(format!("failed building virtual touchpad: {e}")))?;

        Ok(dev)
    }

    pub fn forward_raw(&mut self, events: &[InputEvent]) -> Result<()> {
        if let Some(ref mut tp) = self.touchpad {
            if !events.is_empty() {
                tp.emit(events).map_err(AppError::Io)?;
            }
        }
        Ok(())
    }

    pub fn release_all_slots(&mut self, active_slots: &[usize]) -> Result<()> {
        if let Some(ref mut tp) = self.touchpad {
            let mut evs = Vec::with_capacity(active_slots.len() * 2 + 1);
            for &slot in active_slots {
                evs.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_MT_SLOT.0,
                    slot as i32,
                ));
                evs.push(InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_MT_TRACKING_ID.0,
                    -1,
                ));
            }
            evs.push(InputEvent::new(
                EventType::SYNCHRONIZATION.0,
                SynchronizationCode::SYN_REPORT.0,
                0,
            ));
            tp.emit(&evs).map_err(AppError::Io)?;
        }
        Ok(())
    }

    pub fn tap_key(&mut self, key: KeyCode) -> Result<()> {
        if let Some(ref mut kb) = self.keyboard {
            let press = InputEvent::new(EventType::KEY.0, key.0, 1);
            let release = InputEvent::new(EventType::KEY.0, key.0, 0);
            let syn = InputEvent::new(
                EventType::SYNCHRONIZATION.0,
                SynchronizationCode::SYN_REPORT.0,
                0,
            );
            kb.emit(&[press, syn, release, syn]).map_err(AppError::Io)?;
        }
        Ok(())
    }
}

/// RAII Guard for device exclusive grab.
/// Owns the Device by value and guarantees that the physical device is ungrabbed
/// and returned to the OS if the daemon exits, drops the guard, or encounters an unwind.
pub struct GrabGuard {
    device: Option<Device>,
    active: bool,
}

impl GrabGuard {
    pub fn new(mut device: Device) -> std::io::Result<Self> {
        device.grab()?;
        Ok(Self {
            device: Some(device),
            active: true,
        })
    }

    pub fn ungrab(&mut self) {
        if self.active {
            if let Some(dev) = &mut self.device {
                let _ = dev.ungrab();
            }
            self.active = false;
        }
    }

    pub fn regrab(&mut self) -> std::io::Result<()> {
        if !self.active {
            if let Some(dev) = &mut self.device {
                dev.grab()?;
                self.active = true;
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn device_mut(&mut self) -> &mut Device {
        self.device.as_mut().expect("Device present in GrabGuard")
    }

    #[allow(dead_code)]
    pub fn into_inner(mut self) -> Device {
        self.ungrab();
        self.device.take().expect("Device present in GrabGuard")
    }
}

impl Drop for GrabGuard {
    fn drop(&mut self) {
        self.ungrab();
    }
}

pub enum ActiveDevice {
    Grabbed(GrabGuard),
    Ungrabbed(Device),
}

impl ActiveDevice {
    pub fn device_mut(&mut self) -> &mut Device {
        match self {
            ActiveDevice::Grabbed(g) => g.device_mut(),
            ActiveDevice::Ungrabbed(d) => d,
        }
    }

    pub fn pause(&mut self) {
        if let ActiveDevice::Grabbed(g) = self {
            g.ungrab();
        }
    }

    pub fn resume(&mut self) {
        if let ActiveDevice::Grabbed(g) = self {
            let _ = g.regrab();
        }
    }
}

#[must_use]
pub fn find_touchpad() -> Option<PathBuf> {
    let entries = fs::read_dir("/dev/input").ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name()?.to_str()?;
        if name.starts_with("event") {
            if let Ok(dev) = Device::open(&path) {
                if let Some(abs) = dev.supported_absolute_axes() {
                    if abs.contains(AbsoluteAxisCode::ABS_MT_POSITION_X)
                        && abs.contains(AbsoluteAxisCode::ABS_MT_POSITION_Y)
                        && dev.properties().contains(PropType::POINTER)
                    {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touchpad_metrics_normalization() {
        let metrics = TouchpadMetrics {
            min_x: 0,
            max_x: 1000,
            min_y: 0,
            max_y: 500,
        };

        assert_eq!(metrics.normalize_x(0.0), 0.0);
        assert_eq!(metrics.normalize_x(500.0), 0.5);
        assert_eq!(metrics.normalize_x(1000.0), 1.0);
        assert_eq!(metrics.normalize_x(-50.0), 0.0);
        assert_eq!(metrics.normalize_x(1200.0), 1.0);
    }

    #[test]
    fn test_edge_detection() {
        let metrics = TouchpadMetrics {
            min_x: 0,
            max_x: 1000,
            min_y: 0,
            max_y: 500,
        };

        let ratio = 0.20;
        assert!(metrics.is_left_edge(100.0, ratio));
        assert!(metrics.is_left_edge(200.0, ratio));
        assert!(!metrics.is_left_edge(250.0, ratio));

        assert!(metrics.is_right_edge(900.0, ratio));
        assert!(metrics.is_right_edge(800.0, ratio));
        assert!(!metrics.is_right_edge(750.0, ratio));
    }
}
