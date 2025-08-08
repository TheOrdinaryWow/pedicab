use axum::{Router, routing::get};

use crate::server::controller;

pub fn build_router() -> Router {
    let routes = vec!["/", "/setup", "/static/{*wildcard}", "/assets/{*wildcard}"];

    let mut router = Router::new();
    for route in routes {
        router = router.route(route, get(controller::embed::get));
    }

    router
}
