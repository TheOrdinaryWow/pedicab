use std::{process, str::FromStr};

use anyhow::Context;
use clap::Parser;
#[cfg(feature = "use-mimalloc")]
use mimalloc::MiMalloc;
use pedicab_cli::Cli;
use pedicab_http::AppState;
#[allow(unused_imports)]
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(feature = "use-mimalloc")]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = {
        dotenvy::dotenv().ok();
        Cli::parse()
    };

    let mut filter = EnvFilter::from_str(&cli.global.log_level.to_string()).context("parse env log level failed")?;

    #[cfg(debug_assertions)]
    {
        filter = filter.add_directive("sled::pagecache=off".parse().unwrap());
        filter = filter.add_directive("sled::tree=off".parse().unwrap());
    }
    #[cfg(not(debug_assertions))]
    {
        filter = filter.add_directive("sled=off".parse().unwrap());
    }

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_file(cfg!(debug_assertions))
                .with_line_number(cfg!(debug_assertions))
                .with_thread_names(false)
                .with_thread_ids(false),
        )
        .init();

    #[cfg(feature = "use-mimalloc")]
    {
        use tracing::debug;
        debug!("mimalloc is enabled");
    }

    info!("pedicab is initializing");

    if cli.server.web_mode {
        pedicab_http::start_web_server(cli.server.clone()).await?;
    }

    let db = match pedicab_db::new_db(cli.global.database_path.clone(), cli.global.flush_database_interval) {
        Ok(db) => db,
        Err(e) => {
            error!("error occurred opening database: {e}");
            process::exit(1);
        }
    };


    let dal = pedicab_db::dal::DataAccessLayer::new(db);

    let fm = pedicab_core::manager::ForwardManager::new(dal.clone(), cli.agent.clone()).await;

    info!("pedicab is ready");

    tokio::select! {
      Err(_) = fm.start_polling() => {
        error!("manager polling error");
      },
      Err(err) = pedicab_http::start_api_server(AppState { cli, dal, fm: fm.clone() }) => {
        error!("http server failed: {:?}", err);
        panic!();
      }
    }

    anyhow::Ok(())
}
