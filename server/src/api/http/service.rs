use crate::cache::Cache;
use crate::ingester::Ingester;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::error;

const COMMON_HEADERS: [(&str, &str); 1] = [("Access-Control-Allow-Origin", "*")];
const MEDIA_HEADERS: [(&str, &str); 1] = [("Content-Type", "video/mp4")];

#[derive(Clone)]
pub struct IngesterService {
    ingester: Arc<dyn Ingester + Send + Sync>,
}

impl IngesterService {
    pub fn new(ingester: Arc<dyn Ingester + Send + Sync>) -> Self {
        IngesterService { ingester }
    }

    async fn handle(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, Infallible> {
        self.ingester.ingest(req).await;

        Ok(empty_response(StatusCode::OK))
    }
}

impl Service<Request<Incoming>> for IngesterService {
    type Response = Response<BoxBody<Bytes, Infallible>>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move { this.handle(req).await })
    }
}

#[derive(Clone)]
pub struct TransmitterService {
    cache: Arc<dyn Cache + Send + Sync>,
}

impl TransmitterService {
    pub fn new(cache: Arc<dyn Cache + Send + Sync>) -> Self {
        TransmitterService { cache }
    }

    async fn handle(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, Infallible> {
        let path = req.uri().path();
        let res = self.cache.get(path).await;
        if let Err(e) = res {
            error!("cache: {}", e);
            return Ok(empty_response(StatusCode::INTERNAL_SERVER_ERROR));
        }

        let body = res.unwrap();
        if body.is_none() {
            return Ok(empty_response(StatusCode::NOT_FOUND));
        }
        let body = body.unwrap();

        let mut response = Response::builder().status(StatusCode::OK);
        for header in COMMON_HEADERS {
            response = response.header(header.0, header.1);
        }
        for header in MEDIA_HEADERS {
            response = response.header(header.0, header.1);
        }
        Ok(response.body(body).unwrap())
    }
}

impl Service<Request<Incoming>> for TransmitterService {
    type Response = Response<BoxBody<Bytes, Infallible>>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move { this.handle(req).await })
    }
}

fn empty_response(status: StatusCode) -> Response<BoxBody<Bytes, Infallible>> {
    let mut response = Response::builder().status(status);
    for header in COMMON_HEADERS {
        response = response.header(header.0, header.1);
    }

    response.body(BoxBody::default()).unwrap()
}
