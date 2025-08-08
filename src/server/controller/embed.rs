use std::time::{Duration, SystemTime};

use axum::{
    Json,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use http::HeaderValue;
use serde_json::json;

use crate::server::embed::EmbeddedFile;

pub async fn get(uri: Uri) -> impl IntoResponse {
    let path = uri.path();
    let target_filename = match path {
        ""
        | "/"
        | "/index.html"
        | "/setup"
        | "/setup/index.html"
        | "/setup/"
        // force line break
        | "//" => "index.html",
        path => path.strip_prefix("/").unwrap_or(path),
    };

    match EmbeddedFile::get(target_filename) {
        Some(file) => {
            let mut resp = Response::builder().status(StatusCode::OK).header(
                header::CONTENT_TYPE,
                format!("{}; charset=utf-8", file.metadata.mimetype()),
            );

            if path.starts_with("/static") | path.starts_with("/assets") {
                let expires_http = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(60 * 60 * 24));
                resp = resp
                    .header(header::CACHE_CONTROL, "public, max-age=86400")
                    .header(header::EXPIRES, HeaderValue::from_str(&expires_http).unwrap());
            } else {
                resp = resp.header(header::CACHE_CONTROL, "no-cache");
            }

            resp.body(axum::body::Body::from(file.data)).unwrap()
        }
        None => (StatusCode::NOT_FOUND, Json(json!({"message": "file not found"}))).into_response(),
    }
}
