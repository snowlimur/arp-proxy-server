use crate::storage::FileStorage;
use crate::stream::{FileMetadata, StreamRegistry};
use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Method, Request, Response, StatusCode};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{debug, error, info, warn};

/// Main service for handling CMAF upload requests
#[derive(Debug, Clone)]
pub struct CMAFUploader {
    pub storage: Arc<FileStorage>,
    pub registry: Arc<StreamRegistry>,
}

impl CMAFUploader {
    /// Create a new CMAF uploader service
    pub fn new(storage: Arc<FileStorage>, inactive_timeout: Duration) -> Self {
        Self {
            storage: storage.clone(),
            registry: Arc::new(StreamRegistry::new(storage.clone(), inactive_timeout)),
        }
    }

    /// Handle an incoming HTTP request
    pub async fn handle(
        &self,
        mut req: Request<Incoming>,
    ) -> Result<Response<Empty<Bytes>>, Infallible> {
        if req.method() == Method::DELETE {
            return Ok(self.empty_response(StatusCode::OK));
        }

        if req.method() != Method::PUT {
            info!("Received not allowed request: {}", req.method());
            return Ok(self.empty_response(StatusCode::METHOD_NOT_ALLOWED));
        }

        let start = Instant::now();
        let path = req.uri().path().to_string();
        debug!("Processing request for path: {}", path);

        // Parse the request path parameters
        let params = match RequestParams::from_path(&path) {
            Ok(params) => params,
            Err(e) => {
                warn!("Invalid request path: {}", e);
                return Ok(self.empty_response(StatusCode::BAD_REQUEST));
            }
        };

        // Get or create the stream
        let stream = self.registry.get(&params.stream_name);
        let time_offset = stream.start.elapsed();

        // Get the next sequence number for this stream or quality
        let seq_num: u32 = if params.quality_idx.is_none() {
            stream.next_number()
        } else {
            stream
                .representation(params.quality_idx.unwrap())
                .next_number()
        };

        // Format the filename
        let filename = params.format_filename(seq_num);
        debug!("Writing to filename: {}", filename);

        // Create metadata for this file
        let mut meta = FileMetadata::new(
            time_offset.as_millis() as u32,
            path,
            filename.clone(),
            params.segment.clone(),
        );
        let mut content = Vec::new();
        let mut content_length = 0;

        // Process the request body in chunks
        while let Some(frame_result) = req.frame().await {
            match frame_result {
                Ok(frame) => {
                    if frame.is_data() {
                        let data = frame.into_data().unwrap();
                        let time_offset = start.elapsed();

                        // Record chunk information
                        meta.add_chunk(time_offset.as_millis() as u32, content_length, data.len());
                        content_length += data.len();
                        content.push(data);

                        // Update the stream's last activity time
                        stream.update_last_write();
                    }
                }
                Err(e) => {
                    error!("Error reading request body: {:?}", e);
                    break;
                }
            }
        }

        // Update metadata with total size
        meta.size = content_length;
        debug!("Received {} bytes of content", content_length);

        // Add file metadata to the manifests or quality
        if params.is_manifest {
            stream.add_manifest(meta);
        } else if params.quality_idx.is_some() {
            let quality = stream.representation(params.quality_idx.unwrap());
            if params.is_init {
                quality.set_init(meta);
            } else {
                quality.add_file(meta);
            }
        }

        // Write the content to storage
        match self.storage.write_file(&filename, content).await {
            Ok(_) => Ok(self.empty_response(StatusCode::OK)),
            Err(e) => {
                error!("Failed to write file {}: {}", filename, e);
                // Still return OK to client since we've processed the request
                Ok(self.empty_response(StatusCode::INTERNAL_SERVER_ERROR))
            }
        }
    }

    /// Create an empty HTTP response with the given status code
    fn empty_response(&self, status: StatusCode) -> Response<Empty<Bytes>> {
        Response::builder()
            .status(status)
            .body(Empty::new())
            .unwrap()
    }
}

/// Implement the Hyper Service trait for our uploader
impl Service<Request<Incoming>> for CMAFUploader {
    type Response = Response<Empty<Bytes>>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move { this.handle(req).await })
    }
}

/// Parsed parameters from a request path
#[derive(Debug, Clone)]
struct RequestParams {
    stream_name: String,
    quality_idx: Option<u32>,
    segment: Option<u32>,
    is_manifest: bool,
    is_init: bool,
}

impl RequestParams {
    /// Parse request parameters from a URI path
    /// Expected format: /<stream_id>[/<quality_name>]/<filename>
    fn from_path(path: &str) -> Result<Self, String> {
        let path = path.trim_start_matches('/');
        let parts: Vec<&str> = path.split('/').collect();

        match parts.len() {
            2 => Ok(Self {
                stream_name: parts[0].to_string(),
                quality_idx: None,
                segment: None,
                is_manifest: true,
                is_init: false,
            }),
            3 => {
                let filename = parts[2].strip_suffix(".m4s");
                if filename.is_none() {
                    return Err("Invalid filename: must end with .m4s".to_string());
                }
                let filename = filename.unwrap();

                let quality_idx = parts[1]
                    .parse::<u32>()
                    .map_err(|e| format!("Invalid quality index: {}", e))?;

                let is_init: bool;
                let segment: Option<u32>;
                if filename.eq("init") {
                    is_init = true;
                    segment = None;
                } else {
                    is_init = false;
                    segment = Some(
                        filename
                            .parse::<u32>()
                            .map_err(|e| format!("Invalid segment number: {}", e))?,
                    );
                }

                Ok(Self {
                    stream_name: parts[0].to_string(),
                    quality_idx: Some(quality_idx),
                    is_manifest: false,
                    is_init,
                    segment,
                })
            }
            _ => Err(format!(
                "Invalid path: expected 2-3 parts, got {}",
                parts.len()
            )),
        }
    }

    /// Format a filename based on parameters and sequence number
    fn format_filename(&self, seq: u32) -> String {
        if self.is_manifest {
            return format!("{}/manifests/{}_index.mpd", self.stream_name, seq);
        }

        if self.quality_idx.is_some() {
            if self.segment.is_some() {
                format!(
                    "{}/{}/{}_{}.m4s",
                    self.stream_name,
                    self.quality_idx.unwrap(),
                    seq,
                    self.segment.unwrap()
                )
            } else {
                format!(
                    "{}/{}/{}_init.m4s",
                    self.stream_name,
                    self.quality_idx.unwrap(),
                    seq,
                )
            }
        } else {
            format!("{}/none/{}_none", self.stream_name, seq)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_path_valid_three_parts() {
        let path = "/stream-1/1/00005.m4s";
        let result = RequestParams::from_path(path).unwrap();
        assert_eq!(result.stream_name, "stream-1");
        assert_eq!(result.quality_idx, Some(1));
        assert_eq!(result.segment, Some(5));
        assert_eq!(result.is_manifest, false);
        assert_eq!(result.is_init, false);
    }

    #[test]
    fn from_path_valid_without_leading_slash() {
        let path = "stream-1/2/00004.m4s";
        let result = RequestParams::from_path(path).unwrap();
        assert_eq!(result.stream_name, "stream-1");
        assert_eq!(result.quality_idx, Some(2));
        assert_eq!(result.segment, Some(4));
        assert_eq!(result.is_manifest, false);
        assert_eq!(result.is_init, false);
    }

    #[test]
    fn from_path_valid_two_parts() {
        let path = "/stream-1/index.mpd";
        let result = RequestParams::from_path(path).unwrap();
        assert_eq!(result.stream_name, "stream-1");
        assert_eq!(result.quality_idx, None);
        assert_eq!(result.segment, None);
        assert_eq!(result.is_manifest, true);
        assert_eq!(result.is_init, false);
    }

    #[test]
    fn from_path_invalid_too_many_parts() {
        let path = "/stream1/1/00001.m4s/extra";
        let result = RequestParams::from_path(path);
        assert!(result.is_err());
    }

    #[test]
    fn from_path_invalid_too_few_parts() {
        let path = "/stream-1";
        let result = RequestParams::from_path(path);
        assert!(result.is_err());
    }
}
