use std::time::Duration;

use axum::{
    Router, middleware,
    routing::{get, post},
};
use tower::ServiceBuilder;
use tower_http::{request_id::PropagateRequestIdLayer, timeout::TimeoutLayer};

use crate::server::{
    AppState, controller,
    layer::{
        self,
        request_id::{REQUEST_ID_HEADER_NAME, RequestIdLayer},
    },
};

pub fn build_router(app_state: AppState) -> Router<AppState> {
    Router::<AppState>::new()
        .nest(
            "/api/v1",
            Router::new()
                .nest(
                    "/rules",
                    Router::new()
                        .route(
                            "/",
                            get(controller::rule::get_rules).post(controller::rule::create_rule),
                        )
                        .route(
                            "/{rule_id}",
                            get(controller::rule::get_rule_by_id)
                                .patch(controller::rule::update_rule)
                                .delete(controller::rule::delete_rule),
                        )
                        .route("/{rule_id}/actions/disable", post(controller::rule::disable_rule))
                        .route("/{rule_id}/actions/enable", post(controller::rule::enable_rule)),
                )
                .nest(
                    "/fm",
                    Router::new()
                        .route("/running", get(controller::fm::get_running_rules))
                        .route(
                            "/stats",
                            get(controller::fm::get_stats).delete(controller::fm::reset_stats),
                        )
                        .route(
                            "/stats/{rule_id}",
                            get(controller::fm::get_stat).delete(controller::fm::reset_stat),
                        )
                        .route("/restart/{rule_id}", get(controller::fm::restart_rule)),
                )
                .nest(
                    "/metrics",
                    Router::new()
                        .route("/system", get(controller::metrics::get_system_info))
                        .route("/network", get(controller::metrics::get_network_info))
                        .route("/host", get(controller::metrics::get_host_info)),
                ),
        )
        .layer(
            ServiceBuilder::new()
                .layer(TimeoutLayer::new(Duration::from_secs(10)))
                .layer(RequestIdLayer::new())
                .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER_NAME))
                .layer(middleware::from_fn_with_state(
                    app_state.clone(),
                    layer::bearer::bearer_auth,
                )),
        )
}
