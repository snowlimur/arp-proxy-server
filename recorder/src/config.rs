use serde::{Deserialize, Deserializer};
use std::time::Duration;

/// Main configuration structure for the recorder
#[derive(Debug, Deserialize)]
pub struct Settings {
    pub http: HttpServer,
    pub storage: Storage,
    #[serde(default)]
    pub stream: StreamSettings,
}

/// HTTP server configuration
#[derive(Debug, Deserialize)]
pub struct HttpServer {
    #[serde(default = "HttpServer::default_addr")]
    pub addr: String,
}

impl HttpServer {
    fn default_addr() -> String {
        "0.0.0.0:9091".to_string()
    }
}

/// Storage configuration
#[derive(Debug, Deserialize)]
pub struct Storage {
    pub path: String,
}

/// Stream handling configuration
#[derive(Debug, Default)]
pub struct StreamSettings {
    /// Timeout in seconds to consider a stream inactive
    pub inactive_timeout: Duration,
}

impl<'de> Deserialize<'de> for StreamSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            #[serde(default = "default_timeout_secs")]
            inactive_timeout: u64,
        }

        fn default_timeout_secs() -> u64 {
            5
        }

        let helper = Helper::deserialize(deserializer)?;

        Ok(StreamSettings {
            inactive_timeout: Duration::from_secs(helper.inactive_timeout),
        })
    }
}

impl StreamSettings {
    pub fn default() -> Self {
        Self {
            inactive_timeout: Duration::from_secs(5),
        }
    }
}
