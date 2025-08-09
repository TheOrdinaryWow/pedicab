use std::{net::IpAddr, path::PathBuf};

use clap::{Args, Parser};

#[derive(Parser, Debug, Clone)]
#[command(name = "pedicab", author = "TheOrdinaryWow")]
#[command(version = "0.0.1")]
#[command(about = "A lightweight high performance port forwarder with web panel support.")]
pub struct Cli {
    #[command(flatten)]
    pub server: ServerConfig,

    #[command(flatten)]
    pub agent: AgentConfig,

    #[command(flatten)]
    pub global: GlobalConfig,
}

#[derive(Args, Debug, Clone)]
pub struct GlobalConfig {
    /// Database path
    #[arg(long, env("DATABASE_PATH"), default_value = "pedicab_data", value_parser = clap::value_parser!(PathBuf))]
    pub database_path: PathBuf,

    /// Execute migrations
    #[arg(short = 'M', long, env("DO_MIGRATIONS"))]
    pub do_migrations: bool,

    /// Flush database interval in milliseconds. Database is loaded in memory by default and flushed
    /// to disk at regular intervals. Smaller values will increase the frequency of flushes,
    /// potentially improving data durability but increasing CPU usage.
    #[arg(long, env("FLUSH_DATABASE_INTERVAL"), default_value_t = 10000, value_parser = clap::value_parser!(u64).range(1000..=60000))]
    pub flush_database_interval: u64,

    /// Log level
    #[arg(long, env("LOG_LEVEL"), default_value_t = tracing_subscriber::filter::LevelFilter::INFO)]
    pub log_level: tracing_subscriber::filter::LevelFilter,
}

#[derive(Args, Debug, Clone)]
pub struct ServerConfig {
    /// Server listen host
    #[arg(short = 'H', long, env("HOST"), default_value = "0.0.0.0")]
    pub listen_host: IpAddr,

    /// Server listen port
    #[arg(short = 'P', long, env("PORT"), default_value_t = 8080, value_parser = clap::value_parser!(u16).range(4..))]
    pub listen_port: u16,

    /// Server auth token
    #[arg(short = 'A', long, env("AUTH_TOKEN"))]
    pub auth_token: String,

    /// Serve web panel only
    #[arg(short = 'W', long, env("WEB_MODE"), default_value_t = false)]
    pub web_mode: bool,
}

#[derive(Args, Debug, Clone)]
pub struct AgentConfig {
    // /// Global maximum bandwidth limit in MB per second, disabled if empty
    // #[arg(long, env("BANDWIDTH_LIMIT"), value_parser = clap::value_parser!(u32).range(1..2^20+1))]
    // pub bandwidth_limit: Option<u32>,
    // .
    /// Expanded file descriptor limit, useful if you have many connections (100,000+)
    #[arg(long, env("EXPANDED_NOFILE_LIMIT"), default_value_t = false)]
    pub expanded_nofile_limit: bool,

    /// Metrics interval in milliseconds
    #[arg(long, env("STATS_UPDATE_INTERVAL"), default_value_t = 300, value_parser = clap::value_parser!(u64).range(100..=5000))]
    pub stats_update_interval: u64,

    /// Global maximum connections limit, disabled if empty
    #[arg(long, env("CONNECTIONS_LIMIT"), value_parser = clap::value_parser!(u64).range(1..))]
    pub connections_limit: Option<u64>,

    /// TCP buffer size in KB. This determines how much data can be stored in the TCP send
    /// and receive buffers. Larger buffer sizes can improve throughput, especially on high-latency
    /// connections
    #[arg(long, env("TCP_BUFFER_SIZE"), default_value_t = 8, value_parser = clap::value_parser!(u8).range(2..))]
    pub tcp_buffer_size: u8,
    // /// Enable zero copy feature (Linux only). This allows data transfer directly from disk to
    // /// network without copying through application memory, reducing CPU usage and improving
    // /// performance for large transfers
    // #[arg(long, env("ENABLE_ZERO_COPY"), default_value_t = false)]
    // pub enable_zero_copy: bool,
    // .
}
