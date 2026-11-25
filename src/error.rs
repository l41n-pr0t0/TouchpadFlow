use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),

    #[error("uinput creation failed: {0}")]
    Uinput(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
