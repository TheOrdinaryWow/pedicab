use axum::{
    extract::{Path, State},
    response::IntoResponse,
};
use http::StatusCode;
use tracing::error;
use uuid::Uuid;

use crate::{AppState, data::response::BaseResponse};

pub async fn get_running_rules(State(state): State<AppState>) -> impl IntoResponse {
    let rules = state.fm.get_rules().await;

    BaseResponse::success(rules)
}

pub async fn restart_rule(State(state): State<AppState>, Path(rule_id): Path<Uuid>) -> impl IntoResponse {
    match state.fm.restart_rule(rule_id).await {
        Ok(_) => BaseResponse::success("ok"),
        Err(err) => {
            error!("failed to restart rule: {}", err);
            BaseResponse::anyhow_error(err)
        }
    }
}

pub async fn get_stats(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.fm.get_stats().await;

    BaseResponse::success(stats)
}

pub async fn reset_stats(State(state): State<AppState>) -> impl IntoResponse {
    state.fm.reset_stats().await;

    BaseResponse::success("ok")
}

pub async fn get_stat(State(state): State<AppState>, Path(rule_id): Path<Uuid>) -> impl IntoResponse {
    match state.fm.get_stat(rule_id).await {
        Some(stats) => BaseResponse::success(stats),
        None => BaseResponse::error(StatusCode::BAD_REQUEST, "rule not found"),
    }
}

pub async fn reset_stat(State(state): State<AppState>, Path(rule_id): Path<Uuid>) -> impl IntoResponse {
    match state.fm.reset_stat(rule_id).await {
        Ok(_) => BaseResponse::success("ok"),
        Err(err) => {
            error!("failed to reset stat: {}", err);
            BaseResponse::anyhow_error(err)
        }
    }
}
