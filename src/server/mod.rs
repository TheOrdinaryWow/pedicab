// pub(in crate::server) mod controller;
mod controller;
mod data;
mod layer;
mod router;

use tracing::info;

use crate::{cli::Cli, database::dal::DataAccessLayer, forward::manager::ForwardManager};

#[derive(Clone)]
pub struct AppState {
    pub cli: Cli,
    pub dal: DataAccessLayer,
    pub fm: ForwardManager,
}

pub async fn start_server(app_state: AppState) -> anyhow::Result<()> {
    let router = router::build_router(app_state.clone());

    let listen_addr = format!(
        "{}:{}",
        app_state.cli.server.listen_host, app_state.cli.server.listen_port
    );

    let listener = tokio::net::TcpListener::bind(listen_addr.clone()).await.unwrap();

    info!("http server is listening on {}", listen_addr);

    axum::serve(listener, router).await.unwrap();
    Ok(())
}
