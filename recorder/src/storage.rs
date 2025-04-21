use crate::error::RecorderError;
use bytes::Bytes;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error};

/// Handles all file operations for the recorder
#[derive(Debug, Clone)]
pub struct FileStorage {
    base_path: Arc<PathBuf>,
}

impl FileStorage {
    /// Create a new file storage with the given base path
    pub fn new(path: String) -> Self {
        let base_path = PathBuf::from(path);
        Self {
            base_path: Arc::new(base_path),
        }
    }

    /// Write content to a file within the storage directory
    pub async fn write_file(
        &self,
        relative_path: &str,
        content: Vec<Bytes>,
    ) -> Result<(), RecorderError> {
        let full_path = self.base_path.join(relative_path);

        // Create directory structure if it doesn't exist
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                error!("Failed to create directory {}: {}", parent.display(), e);
                RecorderError::StorageError(format!("Failed to create directory: {}", e))
            })?;
        }

        // Write content to the file
        let mut file_content = Vec::new();
        for chunk in content {
            file_content.extend_from_slice(&chunk);
        }

        tokio::fs::write(&full_path, file_content)
            .await
            .map_err(|e| {
                error!("Failed to write file {}: {}", full_path.display(), e);
                RecorderError::StorageError(format!("Failed to write file: {}", e))
            })?;

        debug!("Successfully wrote file: {}", full_path.display());
        Ok(())
    }
}
