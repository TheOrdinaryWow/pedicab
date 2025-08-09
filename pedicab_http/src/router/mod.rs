mod embed;
mod v1;

use std::time::Duration;

use axum::{Router, routing::get};
use console::style;
use http::{HeaderValue, Request, Response};
use tower_http::{compression::CompressionLayer, normalize_path::NormalizePathLayer, trace::TraceLayer};
use tracing::Span;

use crate::{AppState, layer::request_id::REQUEST_ID_HEADER_NAME};

pub fn build_api_router(app_state: AppState) -> Router {
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &Request<_>| {
            tracing::info_span!(
                "request",
                method = tracing::field::display(request.method()),
                path = tracing::field::display(request.uri())
            )
        })
        .on_response(|response: &Response<_>, elapsed: Duration, _span: &Span| {
            #[inline]
            fn format_duration(duration: &Duration) -> String {
                let nanos = duration.as_nanos();

                if nanos < 1_000 {
                    format!("{}ns", nanos)
                } else if nanos < 1_000_000 {
                    let micros_int = nanos / 1_000;
                    let micros_frac = (nanos % 1_000) / 10;
                    if micros_frac == 0 {
                        format!("{}µs", micros_int)
                    } else {
                        format!("{}.{:02}µs", micros_int, micros_frac)
                    }
                } else if nanos < 1_000_000_000 {
                    let millis_int = nanos / 1_000_000;
                    let millis_frac = (nanos % 1_000_000) / 10_000;
                    if millis_frac == 0 {
                        format!("{}ms", millis_int)
                    } else {
                        format!("{}.{:02}ms", millis_int, millis_frac)
                    }
                } else {
                    let seconds_int = nanos / 1_000_000_000;
                    let seconds_frac = (nanos % 1_000_000_000) / 10_000_000;
                    if seconds_frac == 0 {
                        format!("{}s", seconds_int)
                    } else {
                        format!("{}.{:02}s", seconds_int, seconds_frac)
                    }
                }
            }

            #[inline]
            fn log_response(status: http::StatusCode, elapsed: &Duration, request_id: &str) {
                let status_code = status.as_u16();
                let formatted_elapsed = format_duration(elapsed);

                fn italic(text: &str) -> String {
                    style(text).italic().to_string()
                }

                if status_code > 299 {
                    tracing::warn!(
                        "request handled {}={} {}={} {}={}",
                        italic("status"),
                        status_code,
                        italic("elapsed"),
                        formatted_elapsed,
                        italic("request_id"),
                        request_id,
                    );
                } else {
                    tracing::info!(
                        "request handled {}={} {}={} {}={}",
                        italic("status"),
                        status_code,
                        italic("elapsed"),
                        formatted_elapsed,
                        italic("request_id"),
                        request_id,
                    );
                }
            }

            log_response(
                response.status(),
                &elapsed,
                response
                    .headers()
                    .get(REQUEST_ID_HEADER_NAME)
                    .unwrap_or(&HeaderValue::from_str("unknown").unwrap())
                    .to_str()
                    .unwrap(),
            );
        });

    Router::<AppState>::new()
        .merge(v1::build_router(app_state.clone()))
        .nest("/health", Router::new().route("/", get(|| async { "ok" })))
        .layer(CompressionLayer::new())
        .layer(NormalizePathLayer::trim_trailing_slash())
        .layer(trace_layer)
        .with_state(app_state.clone())
}

pub fn build_web_router() -> Router {
    Router::new()
        .merge(embed::build_router())
        .nest("/health", Router::new().route("/", get(|| async { "ok" })))
        .layer(CompressionLayer::new())
        .layer(NormalizePathLayer::trim_trailing_slash())
}
