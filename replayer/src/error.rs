use std::{error::Error, fmt};

#[derive(Debug)]
pub enum ReplayerError {
    ConfigError(String),
    NetworkError(String),
    StorageError(String),
    RequestError(String),
}

impl fmt::Display for ReplayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplayerError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            ReplayerError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ReplayerError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            ReplayerError::RequestError(msg) => write!(f, "Request error: {}", msg),
        }
    }
}

impl Error for ReplayerError {}
