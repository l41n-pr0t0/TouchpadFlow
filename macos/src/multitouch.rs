use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MTPoint {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MTContact {
    pub frame: i32,
    pub timestamp: f64,
    pub identifier: i32,
    pub state: i32,
    pub finger_id: i32,
    pub hand_id: i32,
    pub normalized_position: MTPoint,
    pub size: f32,
    pub zero1: i32,
    pub angle: f32,
    pub major_axis: f32,
    pub minor_axis: f32,
    pub absolute_position: MTPoint,
    pub zero2: [i32; 2],
    pub velocity: MTPoint,
    pub zero3: [i32; 2],
}

pub type MTContactCallback = extern "C" fn(
    device: *mut c_void,
    contacts: *const MTContact,
    num_contacts: i32,
    timestamp: f64,
    frame: i32,
) -> i32;

pub struct MultitouchDevice {
    device: *mut c_void,
    start_fn: unsafe extern "C" fn(*mut c_void, i32),
    stop_fn: unsafe extern "C" fn(*mut c_void, i32),
}

impl MultitouchDevice {
    pub fn open_default(callback: MTContactCallback) -> Result<Self, String> {
        unsafe {
            let lib = libc::dlopen(
                b"/System/Library/PrivateFrameworks/MultitouchSupport.framework/MultitouchSupport\0".as_ptr() as *const _,
                libc::RTLD_NOW,
            );

            if lib.is_null() {
                return Err("Failed to load MultitouchSupport.framework".into());
            }

            let create_default_fn: unsafe extern "C" fn() -> *mut c_void =
                std::mem::transmute(libc::dlsym(lib, b"MTDeviceCreateDefault\0".as_ptr() as *const _));

            let register_cb_fn: unsafe extern "C" fn(*mut c_void, MTContactCallback) =
                std::mem::transmute(libc::dlsym(lib, b"MTRegisterContactFrameCallback\0".as_ptr() as *const _));

            let start_fn: unsafe extern "C" fn(*mut c_void, i32) =
                std::mem::transmute(libc::dlsym(lib, b"MTDeviceStart\0".as_ptr() as *const _));

            let stop_fn: unsafe extern "C" fn(*mut c_void, i32) =
                std::mem::transmute(libc::dlsym(lib, b"MTDeviceStop\0".as_ptr() as *const _));

            let device = create_default_fn();
            if device.is_null() {
                return Err("No built-in multitouch trackpad found".into());
            }

            register_cb_fn(device, callback);
            start_fn(device, 0);

            Ok(Self {
                device,
                start_fn,
                stop_fn,
            })
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
