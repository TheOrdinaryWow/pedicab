use std::{
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use serde_json::json;
use tracing::error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseResponse<T> {
    Success(T),
    Error { code: StatusCode, message: String },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorResponseContent {
    message: String,
}

impl<T> BaseResponse<T>
where
    T: Debug,
{
    #[allow(dead_code)]
    pub fn success(data: T) -> Self {
        Self::Success(data)
    }

    #[allow(dead_code)]
    pub fn error(code: StatusCode, message: impl ToString + Debug) -> Self {
        let message = message.to_string();
        error!("Response error. code: {}, message: {}", code, message);
        Self::Error { code, message }
    }

    #[allow(dead_code)]
    pub fn server_error(error: impl Error) -> Self {
        error!("Response server error: {}", error);
        Self::Error {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn anyhow_error(error: anyhow::Error) -> Self {
        error!("Response error: {}", error);
        Self::Error {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl Default for BaseResponse<()> {
    fn default() -> Self {
        Self::Success(())
    }
}

impl<E> From<E> for BaseResponse<()>
where
    E: Error,
{
    fn from(err: E) -> Self {
        Self::server_error(err)
    }
}

impl<T> Display for BaseResponse<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BaseResponse::Success(data) => write!(f, "BaseResponse::Success({:?})", data),
            BaseResponse::Error { code, message } => {
                write!(f, "BaseResponse::Error {{ code: {}, message: {} }}", code, message)
            }
        }
    }
}

impl<T> IntoResponse for BaseResponse<T>
where
    T: Serialize + Send + Sync + 'static,
{
    fn into_response(self) -> Response {
        match self {
            BaseResponse::Success(data) => (StatusCode::OK, Json(json!({"data": data}))).into_response(),

            BaseResponse::Error { code, message } => {
                let body = Json(ErrorResponseContent { message });
                (code, body).into_response()
            }
        }
    }
}
