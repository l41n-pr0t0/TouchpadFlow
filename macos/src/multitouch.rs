use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MTPoint {
    pub x: f32,
    pub y: f32,
}

/// Mirrors Apple's private `MTTouch` (96 bytes). A wrong size or field order corrupts every
/// finger after the first, because the callback hands us a packed array of these.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MTContact {
    pub frame: i32,
    pub timestamp: f64,
    pub identifier: i32,
    /// 0 untracked, 1 start, 2 hover, 3 make-touch, 4 touching, 5 break, 6 linger, 7 out of range.
    pub state: i32,
    pub finger_id: i32,
    pub hand_id: i32,
    pub normalized_position: MTPoint,
    pub normalized_velocity: MTPoint,
    pub size: f32,
    pub zero1: i32,
    pub angle: f32,
    pub major_axis: f32,
    pub minor_axis: f32,
    pub absolute_position: MTPoint,
    pub absolute_velocity: MTPoint,
    pub zero2: [i32; 2],
    pub density: f32,
}

const _: () = assert!(std::mem::size_of::<MTContact>() == 96);

pub type MTContactCallback = extern "C" fn(
    device: *mut c_void,
    contacts: *const MTContact,
    num_contacts: i32,
    timestamp: f64,
    frame: i32,
) -> i32;

pub struct MultitouchDevice {
    device: *mut c_void,
    stop_fn: unsafe extern "C" fn(*mut c_void, i32),
}

impl MultitouchDevice {
    pub fn open_default(callback: MTContactCallback) -> Result<Self, String> {
        unsafe {
            let lib = libc::dlopen(
                b"/System/Library/PrivateFrameworks/MultitouchSupport.framework/MultitouchSupport\0"
                    .as_ptr() as *const _,
                libc::RTLD_NOW,
            );

            if lib.is_null() {
                return Err("Failed to load MultitouchSupport.framework".into());
            }

            // A missing symbol would be a null fn pointer (UB to call): fail cleanly instead.
            let sym = |name: &[u8]| {
                let p = libc::dlsym(lib, name.as_ptr() as *const _);
                if p.is_null() {
                    Err(format!(
                        "MultitouchSupport is missing {}",
                        String::from_utf8_lossy(&name[..name.len() - 1])
                    ))
                } else {
                    Ok(p)
                }
            };

            let create_default_fn: unsafe extern "C" fn() -> *mut c_void =
                std::mem::transmute(sym(b"MTDeviceCreateDefault\0")?);

            let register_cb_fn: unsafe extern "C" fn(*mut c_void, MTContactCallback) =
                std::mem::transmute(sym(b"MTRegisterContactFrameCallback\0")?);

            let start_fn: unsafe extern "C" fn(*mut c_void, i32) =
                std::mem::transmute(sym(b"MTDeviceStart\0")?);

            let stop_fn: unsafe extern "C" fn(*mut c_void, i32) =
                std::mem::transmute(sym(b"MTDeviceStop\0")?);

            let device = create_default_fn();
            if device.is_null() {
                return Err("No built-in multitouch trackpad found".into());
            }

            register_cb_fn(device, callback);
            start_fn(device, 0);

            Ok(Self { device, stop_fn })
        }
    }
}

impl Drop for MultitouchDevice {
    fn drop(&mut self) {
        unsafe {
            (self.stop_fn)(self.device, 0);
        }
    }
}
