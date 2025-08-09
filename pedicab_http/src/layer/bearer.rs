use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use http::{StatusCode, header};
use serde_json::json;

use crate::AppState;

pub async fn bearer_auth(
    State(state): State<AppState>,
    // `Request` does
    request: Request,
    next: Next,
) -> Response {
    let token = match request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        Some(token) => token,
        None => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "message": "missing authentication token"
                    })
                    .to_string(),
                ))
                .unwrap();
        }
    };

    let token = match token.strip_prefix("Bearer ") {
        Some(token) => token,
        None => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "message": "malformatted authentication token"
                    })
                    .to_string(),
                ))
                .unwrap();
        }
    };

    if token != state.cli.server.auth_token {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "message": "invalid authentication token"
                })
                .to_string(),
            ))
            .unwrap();
    }

    next.run(request).await
}
