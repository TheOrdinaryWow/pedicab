// pub(in crate::server) mod controller;
mod controller;
mod data;
mod embed;
mod layer;
mod router;

use pedicab_cli::{Cli, ServerConfig};
use pedicab_core::manager::ForwardManager;
use pedicab_db::dal::DataAccessLayer;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub cli: Cli,
    pub dal: DataAccessLayer,
    pub fm: ForwardManager,
}

pub async fn start_api_server(app_state: AppState) -> anyhow::Result<()> {
    let router = router::build_api_router(app_state.clone());

    let listen_addr = format!(
        "{}:{}",
        app_state.cli.server.listen_host, app_state.cli.server.listen_port
    );

    let listener = tokio::net::TcpListener::bind(listen_addr.clone()).await.unwrap();

    info!("http server is listening on {}", listen_addr);

    axum::serve(listener, router).await.unwrap();
    Ok(())
}

pub async fn start_web_server(config: ServerConfig) -> anyhow::Result<()> {
    let router = router::build_web_router();

    let listen_addr = format!("{}:{}", config.listen_host, config.listen_port);

    let listener = tokio::net::TcpListener::bind(listen_addr.clone()).await.unwrap();

    info!("http server is listening on {} with web mode enabled", listen_addr);

    axum::serve(listener, router).await.unwrap();
    Ok(())
}
