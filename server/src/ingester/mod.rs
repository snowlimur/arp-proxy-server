use async_trait::async_trait;
use hyper::body::Incoming;
use hyper::Request;

pub mod list_ingester;
pub mod map_ingester;
pub mod simple_ingester;

#[async_trait]
pub trait Ingester {
    async fn ingest(&self, req: Request<Incoming>);
}
