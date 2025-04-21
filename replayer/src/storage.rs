use crate::error::ReplayerError;
use crate::stream::{FileMetadata, StreamMetadata};
use bytes::Bytes;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Handles all file operations for the re-player
#[derive(Debug, Clone)]
pub struct FileStorage {
    base_path: PathBuf,
    file_cache: HashMap<String, Bytes>,
    meta_cache: HashMap<String, Arc<StreamMetadata>>,
}

impl FileStorage {
    /// Create a new file storage with the given base path
    pub fn new(path: String) -> Self {
        let base_path = PathBuf::from(path);
        Self {
            base_path,
            file_cache: HashMap::new(),
            meta_cache: HashMap::new(),
        }
    }

    pub async fn read_metadata(&mut self, stream: &str) -> Result<(), ReplayerError> {
        if self.meta_cache.contains_key(stream) {
            return Ok(());
        }

        let path = self.base_path.join(stream).join("metadata.json");

        let file_content = tokio::fs::read(&path)
            .await
            .map_err(|e| ReplayerError::StorageError(format!("Failed to read file: {}", e)))?;

        let mut metadata: StreamMetadata = serde_json::from_slice(&file_content).map_err(|e| {
            ReplayerError::StorageError(format!("Failed to deserialize meta content: {}", e))
        })?;

        self.read_files(&mut metadata).await?;
        self.meta_cache
            .insert(stream.to_string(), Arc::new(metadata.clone()));

        Ok(())
    }

    pub fn get_metadata(&self, stream: &str) -> Option<Arc<StreamMetadata>> {
        self.meta_cache.get(stream).cloned()
    }

    async fn read_files(&mut self, metadata: &mut StreamMetadata) -> Result<(), ReplayerError> {
        for manifest in metadata.manifests.iter() {
            self.read_file(manifest.clone()).await?;
        }

        for representation in metadata.representations.iter() {
            if representation.init.is_some() {
                let init = representation.init.clone().unwrap();
                self.read_file(init).await?;
            }

            for file in representation.segments.iter() {
                self.read_file(file.clone()).await?;
            }
        }

        Ok(())
    }

    async fn read_file(&mut self, file: Arc<FileMetadata>) -> Result<(), ReplayerError> {
        let path = self.base_path.join(&file.file_name);
        let file_content = tokio::fs::read(&path).await.map_err(|e| {
            ReplayerError::StorageError(format!(
                "Failed to read segment file {}: {}",
                file.file_name, e
            ))
        })?;

        let data = Bytes::copy_from_slice(&file_content);
        self.file_cache.insert(file.file_name.clone(), data);

        Ok(())
    }

    pub fn get_file(&self, file_name: &str) -> Option<Bytes> {
        self.file_cache.get(file_name).cloned()
    }
}
