use std::{error::Error, fmt};

#[derive(Debug)]
pub enum RecorderError {
    ConfigError(String),
    NetworkError(String),
    StorageError(String),
    RequestError(String),
}

impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecorderError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            RecorderError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            RecorderError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            RecorderError::RequestError(msg) => write!(f, "Request error: {}", msg),
        }
    }
}

impl Error for RecorderError {}
