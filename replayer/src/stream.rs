use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Metadata for an entire stream - used for JSON serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMetadata {
    pub name: String,
    pub manifests: Vec<Arc<FileMetadata>>,
    pub representations: Vec<Arc<RepresentationMetadata>>,
}

/// Metadata for a quality in a stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepresentationMetadata {
    pub idx: u32,
    pub init: Option<Arc<FileMetadata>>,
    pub segments: Vec<Arc<FileMetadata>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: String,
    pub file_name: String,
    pub segment: Option<u32>,
    pub time_offset: u32,                 // ms from stream start
    pub size: usize,                      // total size in bytes
    pub chunks: Vec<(u32, usize, usize)>, // (time offset in ms, bytes offset, size in bytes)
}
