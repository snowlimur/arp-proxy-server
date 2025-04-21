use crate::storage::FileStorage;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::collections::HashMap;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicU32, AtomicU64};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

/// Registry for managing multiple active streams
#[derive(Debug, Clone)]
pub struct StreamRegistry {
    inactive_timeout: u64, // in milliseconds
    streams: Arc<RwLock<HashMap<String, Arc<Stream>>>>,
    storage: Arc<FileStorage>,
}

impl StreamRegistry {
    /// Create a new stream registry with the given inactive timeout
    pub fn new(storage: Arc<FileStorage>, inactive_timeout: Duration) -> Self {
        Self {
            inactive_timeout: inactive_timeout.as_millis() as u64,
            streams: Arc::new(RwLock::new(HashMap::new())),
            storage,
        }
    }

    /// Get or create a stream with the given ID
    pub fn get(&self, stream_id: &str) -> Arc<Stream> {
        // Try to get an existing stream
        let mut registry = self.streams.write().unwrap();
        if let Some(stream) = registry.get(stream_id) {
            debug!("Retrieved existing stream: {}", stream_id);
            return stream.clone();
        }

        // Create a new stream
        info!("Creating new stream: {}", stream_id);
        let stream = Arc::new(Stream::new(stream_id.to_string()));
        registry.insert(stream_id.to_string(), stream.clone());

        // Start a watchdog task to monitor inactivity
        let streams = self.streams.clone();
        let storage = self.storage.clone();
        let stream_clone = stream.clone();
        let inactive_timeout = self.inactive_timeout;
        tokio::spawn(async move {
            Self::watchdog(stream_clone, streams, storage, inactive_timeout).await;
        });

        stream
    }

    /// Watchdog task that monitors stream activity and removes inactive streams
    async fn watchdog(
        stream: Arc<Stream>,
        streams: Arc<RwLock<HashMap<String, Arc<Stream>>>>,
        storage: Arc<FileStorage>,
        timeout: u64,
    ) {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;

            let elapsed = stream.start.elapsed().as_millis() as u64;
            let last_write = stream.last_write.load(Relaxed);

            if elapsed - last_write > timeout {
                // Move the stream ID to a local variable for reference after removal
                let stream_id = stream.id.clone();

                // Remove the stream from the registry - this needs to be done in a separate scope
                // to ensure the write lock is released before await
                {
                    let mut registry = streams.write().unwrap();
                    registry.remove(&stream_id);
                    info!("Stream \"{}\" removed due to inactivity", stream_id);
                }

                // Write all metadata of the stream to a JSON file on disk
                let metadata_path = format!("{}/metadata.json", stream_id);
                // Create a serializable metadata structure
                let metadata = stream.export_metadata();
                // Serialize to JSON
                let json = serde_json::to_string_pretty(&metadata);
                if let Err(e) = json.as_ref() {
                    error!("Serialize stream metadata {}: {}", stream_id, e);
                    return;
                }

                let json = json.unwrap();
                let mut data = Vec::new();
                data.push(Bytes::from(json));

                if let Err(e) = storage.write_file(metadata_path.as_str(), data).await {
                    error!("Saving stream metadata for {}: {}", stream_id, e);
                } else {
                    debug!("Stream metadata saved to {}", metadata_path);
                }

                break;
            }
        }
    }
}

/// Represents a single content stream with multiple quality variants
#[derive(Debug)]
pub struct Stream {
    pub id: String,
    pub start: Instant,
    pub last_write: AtomicU64,
    manifests: Arc<RwLock<Vec<FileMetadata>>>,
    representations: Arc<RwLock<HashMap<u32, Arc<Representation>>>>,
    counter: AtomicU32,
}

impl Stream {
    /// Create a new stream with the given ID
    fn new(id: String) -> Self {
        let now = Instant::now();
        Self {
            id,
            start: now,
            counter: AtomicU32::new(0),
            last_write: AtomicU64::new(0),
            manifests: Arc::new(RwLock::new(Vec::new())),
            representations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the next sequence number for this stream
    pub fn next_number(&self) -> u32 {
        self.counter.fetch_add(1, Relaxed)
    }

    /// Get or create a quality variant for this stream
    pub fn representation(&self, idx: u32) -> Arc<Representation> {
        let mut representations = self.representations.write().unwrap();
        if let Some(representation) = representations.get(&idx) {
            return representation.clone();
        }

        // Create a new quality variant
        debug!(
            "Creating new representation variant '{}' for stream '{}'",
            idx, self.id
        );
        let representation = Arc::new(Representation::new());
        representations.insert(idx, representation.clone());
        representation
    }

    /// Add a file to this stream
    pub fn add_manifest(&self, file: FileMetadata) {
        let mut files = self.manifests.write().unwrap();
        files.push(file);
    }

    /// Update the last write time for this stream
    pub fn update_last_write(&self) {
        let elapsed = self.start.elapsed();
        self.last_write.store(elapsed.as_millis() as u64, Relaxed);
    }

    pub fn export_metadata(&self) -> StreamMetadata {
        let mut representations: Vec<RepresentationMetadata> = Vec::new();
        for (name, representation) in self.representations.read().unwrap().iter() {
            let meta = RepresentationMetadata {
                idx: name.clone(),
                init: representation.init.read().unwrap().clone(),
                segments: representation.segments(),
            };

            representations.push(meta);
        }

        StreamMetadata {
            name: self.id.clone(),
            manifests: self.manifests.read().unwrap().clone(),
            representations,
        }
    }
}

/// Represents a specific quality variant of a stream
#[derive(Debug)]
pub struct Representation {
    counter: AtomicU32,
    init: RwLock<Option<FileMetadata>>,
    segments: RwLock<Vec<FileMetadata>>,
}

impl Representation {
    fn new() -> Self {
        Self {
            init: RwLock::new(None),
            counter: AtomicU32::new(0),
            segments: RwLock::new(Vec::new()),
        }
    }

    /// Get the next sequence number for this quality variant
    pub fn next_number(&self) -> u32 {
        self.counter.fetch_add(1, Relaxed)
    }

    /// Set the initialization segment for this quality variant
    pub fn set_init(&self, file: FileMetadata) {
        self.init.write().unwrap().replace(file);
    }

    /// Add a file to this quality variant
    pub fn add_file(&self, file: FileMetadata) {
        let mut files = self.segments.write().unwrap();
        files.push(file);
    }

    /// Get all files for this quality variant
    pub fn segments(&self) -> Vec<FileMetadata> {
        let files = self.segments.read().unwrap();
        files.clone()
    }
}

/// Metadata for an entire stream - used for JSON serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMetadata {
    pub name: String,
    pub manifests: Vec<FileMetadata>,
    pub representations: Vec<RepresentationMetadata>,
}

/// Metadata for a quality in a stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepresentationMetadata {
    pub idx: u32,
    pub init: Option<FileMetadata>,
    pub segments: Vec<FileMetadata>,
}

/// Metadata for a file in a stream
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: String,
    pub file_name: String,
    pub segment: Option<u32>,
    pub time_offset: u32,                 // ms from stream start
    pub size: usize,                      // total size in bytes
    pub chunks: Vec<(u32, usize, usize)>, // (time offset in ms, byte offset, size in bytes)
}

impl FileMetadata {
    /// Create new file metadata
    pub fn new(offset: u32, path: String, file_name: String, segment: Option<u32>) -> Self {
        Self {
            file_name,
            path,
            segment,
            chunks: Vec::new(),
            time_offset: offset,
            size: 0,
        }
    }

    /// Add a chunk to this file's metadata
    pub fn add_chunk(&mut self, time_offset: u32, byte_offset: usize, size: usize) {
        self.chunks.push((time_offset, byte_offset, size));
        self.size += size;
    }
}
