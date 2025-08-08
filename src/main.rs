mod cli;
mod database;
mod forward;
mod server;

use std::{process, str::FromStr};

use anyhow::Context;
use clap::Parser;
use cli::Cli;
use mimalloc::MiMalloc;
#[allow(unused_imports)]
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{database::dal, server::AppState};


#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = {
        dotenvy::dotenv().ok();
        Cli::parse()
    };

    #[allow(unused_mut)]
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

    info!("pedicab is initializing");

    if cli.server.web_mode {
        server::start_web_server(cli.server.clone()).await?;
    }

    let db = match sled::Config::new()
        .path(cli.global.database_path.clone())
        .cache_capacity(1024 * 1024 * 8)
        .flush_every_ms(Some(cli.global.flush_database_interval))
        .mode(sled::Mode::HighThroughput)
        .open()
    {
        Ok(db) => db,
        Err(e) => {
            error!("error occurred opening database: {e}");
            process::exit(1);
        }
    };


    let dal = dal::DataAccessLayer::new(db);

    let fm = forward::manager::ForwardManager::new(dal.clone(), cli.agent.clone()).await;

    info!("pedicab is ready");

    tokio::select! {
      Err(_) = fm.start_polling() => {
        error!("manager polling error");
      },
      Err(err) = server::start_api_server(AppState { cli, dal, fm: fm.clone() }) => {
        error!("http server failed: {:?}", err);
        panic!();
      }
    }

    anyhow::Ok(())
}
