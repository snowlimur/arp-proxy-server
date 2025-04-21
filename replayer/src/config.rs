use serde::Deserialize;
use std::time::Duration;

/// Main configuration structure for the recorder
#[derive(Debug, Deserialize)]
pub struct Settings {
    pub target: Target,
    pub storage: Storage,
    pub schedule: Schedule,
}

/// Target configuration
#[derive(Debug, Deserialize)]
pub struct Target {
    pub url: String,
}

/// Storage configuration
#[derive(Debug, Deserialize)]
pub struct Storage {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct Schedule {
    pub steps: Vec<ScheduleStep>,
}

#[derive(Debug, Deserialize)]
pub struct ScheduleStep {
    #[serde(default, with = "humantime_serde")]
    pub duration: Option<Duration>,
    #[serde(default, with = "humantime_serde")]
    pub delay: Option<Duration>,
    pub stream: String,
    pub parallel: u32,
}
