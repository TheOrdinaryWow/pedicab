use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use pedicab_db::dal::rule::{CreateRuleParams, UpdateRuleParams};
use tracing::error;
use uuid::Uuid;

use crate::{
    AppState,
    data::{body::InputBody, response::BaseResponse},
};

pub async fn get_rules(State(state): State<AppState>) -> impl IntoResponse {
    match state.dal.rule.find_all().await {
        Ok(rules) => BaseResponse::success(rules),
        Err(err) => {
            error!("failed to fetch rules: {}", err);
            BaseResponse::server_error(err)
        }
    }
}

pub async fn get_rule_by_id(State(state): State<AppState>, Path(rule_id): Path<Uuid>) -> impl IntoResponse {
    match state.dal.rule.find_by_id(rule_id).await {
        Ok(rule) => BaseResponse::success(rule),
        Err(err) => {
            error!("failed to fetch rule by id: {}", err);
            BaseResponse::server_error(err)
        }
    }
}

pub async fn create_rule(
    State(state): State<AppState>, Json(body): Json<InputBody<CreateRuleParams>>,
) -> impl IntoResponse {
    match state.dal.rule.create(body.data).await {
        Ok(rule) => BaseResponse::success(rule),
        Err(err) => {
            error!("failed to create rule: {}", err);
            BaseResponse::server_error(err)
        }
    }
}

pub async fn update_rule(
    State(state): State<AppState>, Path(rule_id): Path<Uuid>, Json(body): Json<InputBody<UpdateRuleParams>>,
) -> impl IntoResponse {
    match state.dal.rule.update(rule_id, body.data).await {
        Ok(rule) => BaseResponse::success(rule),
        Err(err) => {
            error!("failed to update rule by id: {}", err);
            BaseResponse::server_error(err)
        }
    }
}

pub async fn delete_rule(State(state): State<AppState>, Path(rule_id): Path<Uuid>) -> impl IntoResponse {
    match state.dal.rule.delete(rule_id).await {
        Ok(_) => BaseResponse::success("ok"),
        Err(err) => {
            error!("failed to delete rule by id: {}", err);
            BaseResponse::server_error(err)
        }
    }
}

pub async fn enable_rule(State(state): State<AppState>, Path(rule_id): Path<Uuid>) -> impl IntoResponse {
    match state.dal.rule.enable(rule_id).await {
        Ok(_) => BaseResponse::success("ok"),
        Err(err) => {
            error!("failed to enable rule by id: {}", err);
            BaseResponse::server_error(err)
        }
    }
}

pub async fn disable_rule(State(state): State<AppState>, Path(rule_id): Path<Uuid>) -> impl IntoResponse {
    match state.dal.rule.disable(rule_id).await {
        Ok(_) => BaseResponse::success("ok"),
        Err(err) => {
            error!("failed to disable rule by id: {}", err);
            BaseResponse::server_error(err)
        }
    }
}
