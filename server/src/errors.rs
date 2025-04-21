use std::{error::Error, fmt};

#[allow(dead_code)]
#[derive(Debug)]
pub enum ServerError {
    ConfigError(String),
    NetworkError(String),
    StorageError(String),
    RequestError(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            ServerError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ServerError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            ServerError::RequestError(msg) => write!(f, "Request error: {}", msg),
        }
    }
}

impl Error for ServerError {}
