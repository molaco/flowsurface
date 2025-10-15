use std::path::PathBuf;
use std::{fs, io};

use crate::data_path;

const LOG_FILE: &str = "flowsurface-current.log";

pub fn file() -> Result<fs::File, Error> {
    let path = path()?;

    Ok(fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(false)
        .truncate(true)
        .open(path)?)
}

pub fn path() -> Result<PathBuf, Error> {
    let full_path = data_path(Some(LOG_FILE));

    let parent = full_path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid log file path"))?;

    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }

    Ok(full_path)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("Failed to set logger: {0}")]
    SetLog(log::SetLoggerError),
    #[error("Failed to parse log level: {0}")]
    ParseLevel(log::ParseLevelError),
}

impl From<log::SetLoggerError> for Error {
    fn from(err: log::SetLoggerError) -> Self {
        Error::SetLog(err)
    }
}

impl From<log::ParseLevelError> for Error {
    fn from(err: log::ParseLevelError) -> Self {
        Error::ParseLevel(err)
    }
}
