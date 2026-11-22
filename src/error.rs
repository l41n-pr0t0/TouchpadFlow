use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),

    #[allow(dead_code)]
    #[error("failed to open device at '{path}': {source}")]
    DeviceOpen {
        path: String,
        #[source]
        source: io::Error,
    },

    #[allow(dead_code)]
    #[error("device '{0}' lacks multitouch absolute axis support")]
    NonMultitouch(String),

    #[error("uinput creation failed: {0}")]
    Uinput(String),

    #[allow(dead_code)]
    #[error("configuration error: {0}")]
    Config(String),

    #[allow(dead_code)]
    #[error("no suitable multitouch touchpad found")]
    DeviceNotFound,
}

pub type Result<T> = std::result::Result<T, AppError>;
