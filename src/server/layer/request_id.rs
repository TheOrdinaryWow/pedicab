use axum::extract::Request;
use http::{HeaderName, HeaderValue};
use tower_http::request_id::{MakeRequestId, RequestId, SetRequestIdLayer};
use uuid::Uuid;

pub const REQUEST_ID_HEADER_NAME: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Clone, Default)]
pub struct RequestIdLayer;

impl MakeRequestId for RequestIdLayer {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        Some(RequestId::new(
            HeaderValue::from_str(&Uuid::now_v7().to_string()).unwrap(),
        ))
    }
}

impl RequestIdLayer {
    pub fn new() -> SetRequestIdLayer<RequestIdLayer> {
        SetRequestIdLayer::new(REQUEST_ID_HEADER_NAME, RequestIdLayer)
    }
}
