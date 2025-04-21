use crate::errors::ServerError;
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use std::convert::Infallible;

pub mod list_cache;
pub mod map_cache;
pub mod static_cache;

#[async_trait]
pub trait Cache {
    async fn get(&self, key: &str) -> Result<Option<BoxBody<Bytes, Infallible>>, ServerError>;
}
