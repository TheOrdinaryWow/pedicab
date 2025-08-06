use std::{
    cmp::min,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tracing::{debug, error, info_span, trace, warn};

use crate::{
    cli::AgentConfig,
    database::{
        dal::DataAccessLayer,
        data::rule::{RuleStats, RuleStatsConnections, RuleStatus},
        model::rule::Rule,
    },
    forward::manager::StatsCache,
};

pub async fn start_tcp_forward(rule: Rule, config: AgentConfig, dal: DataAccessLayer, stats_cache: StatsCache) {
    let span = info_span!("start_tcp_forward", rule_id = rule.id.as_uuid().to_string());

    let listener = match TcpListener::bind(rule.listen).await {
        Ok(listener) => {
            if let Err(e) = listener.set_ttl(255) {
                debug!(parent: &span, "failed to set TTL: {}", e);
            }
            listener
        }
        Err(e) => {
            error!(parent: &span, "failed to bind to {}: {}", rule.listen, e);

            // handle non-retryable errors
            {
                let updated_stats = match stats_cache.get(&rule.id.into()).await {
                    Some(current_stats) => RuleStats {
                        last_failed_message: format!("failed to bind to {}", rule.listen),
                        ..current_stats
                    },
                    None => RuleStats {
                        last_failed_message: format!("failed to bind to {}", rule.listen),
                        ..Default::default()
                    },
                };
                let _ = dal.rule.update_status(rule.id.into(), RuleStatus::Error).await;
                stats_cache.insert(rule.id.into(), updated_stats).await;
            }

            return;
        }
    };

    let connections_semaphore = {
        let connections_limit = match (rule.config.connections, config.connections_limit) {
            (Some(rule_limit), Some(config_limit)) => Some(min(rule_limit, config_limit)),
            (Some(rule_limit), None) => Some(rule_limit),
            (None, Some(config_limit)) => Some(config_limit),
            (None, None) => None,
        };

        connections_limit.map(|conn| Arc::new(tokio::sync::Semaphore::new(conn as usize)))
    };

    debug!(parent: &span, "tcp forwarding started on {}",rule.listen);

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                let sem_permit = match &connections_semaphore {
                    Some(semaphore) => match semaphore.clone().try_acquire_owned() {
                        Ok(permit) => Some(permit),
                        Err(_) => {
                            warn!(parent: &span, "max connections reached, rejecting connection from {}", addr);
                            continue;
                        }
                    },
                    None => None,
                };

                trace!(parent: &span, "new connection from {}", addr);

                let target_addr = rule.target.addrs[0];
                let rule_id = rule.id;
                let config = config.clone();
                let stats_cache = stats_cache.clone();

                tokio::spawn(async move {
                    let _permit = sem_permit;

                    handle_connection(socket, target_addr, rule_id.as_uuid(), config, stats_cache).await;
                });
            }
            Err(e) => {
                error!(parent: &span, "failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_connection(
    mut client_stream: TcpStream, target_addr: std::net::SocketAddr, rule_id: uuid::Uuid, config: AgentConfig,
    stats_cache: StatsCache,
) {
    let span = info_span!(
        "handle_tcp_connection",
        rule_id = rule_id.to_string(),
        target_addr = target_addr.to_string()
    );

    let buffer_size = (config.tcp_buffer_size as usize) * 1024;

    {
        let prev_stats = stats_cache.get(&rule_id).await.unwrap_or_default();
        stats_cache
            .insert(
                rule_id,
                RuleStats {
                    connections: RuleStatsConnections {
                        tcp: prev_stats.connections.tcp + 1,
                        ..prev_stats.connections
                    },
                    ..prev_stats
                },
            )
            .await;
    }

    match TcpStream::connect(target_addr).await {
        Ok(mut server_stream) => {
            trace!(parent: &span, "connected to target");

            {
                let client_sock_ref = socket2::SockRef::from(&client_stream);
                let server_sock_ref = socket2::SockRef::from(&server_stream);

                // Set TCP keepalive
                {
                    let mut ka = socket2::TcpKeepalive::new();
                    ka = ka.with_time(Duration::from_secs(20));
                    ka = ka.with_interval(Duration::from_secs(20));

                    if let Err(e) = client_sock_ref.set_tcp_keepalive(&ka) {
                        debug!(parent: &span, "failed to set tcp keepalive for client stream: {}", e);
                    }
                    if let Err(e) = server_sock_ref.set_tcp_keepalive(&ka) {
                        debug!(parent: &span, "failed to set tcp keepalive for server stream: {}", e);
                    }
                }

                // Set TCP window size
                {
                    if let Err(e) = client_sock_ref.set_recv_buffer_size(buffer_size * 4) {
                        debug!(parent: &span, "failed to set client recv buffer: {}", e);
                    }
                    if let Err(e) = server_sock_ref.set_send_buffer_size(buffer_size * 4) {
                        debug!(parent: &span, "failed to set server send buffer: {}", e);
                    }
                }

                // Set TCP no delay
                {
                    if let Err(e) = client_sock_ref.set_tcp_nodelay(true) {
                        debug!(parent: &span, "failed to set nodelay for client stream: {}", e);
                    }
                    if let Err(e) = server_sock_ref.set_tcp_nodelay(true) {
                        debug!(parent: &span, "failed to set nodelay for server stream: {}", e);
                    }
                }
            }

            let (mut client_reader, mut client_writer) = client_stream.split();
            let (mut server_reader, mut server_writer) = server_stream.split();

            let transferred_bytes = Arc::new(AtomicU64::new(0));
            let last_update_time = Arc::new(Mutex::new(tokio::time::Instant::now()));

            let stats_update_interval = Duration::from_millis(config.stats_update_interval);
            let stats_updater = {
                let transferred_bytes = transferred_bytes.clone();
                let last_update_time = last_update_time.clone();
                let stats_cache = stats_cache.clone();

                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(stats_update_interval);

                    loop {
                        interval.tick().await;

                        let prev_stats = stats_cache.get(&rule_id).await.unwrap_or_default();

                        // Get the number of bytes transferred since the last update
                        let bytes = transferred_bytes.swap(0, Ordering::Relaxed);

                        // Calculate total bandwidth
                        let bandwidth = prev_stats.bandwidth + bytes;

                        // Calculation speed (bytes/second)
                        let mut speed = prev_stats.speed;

                        let mut last_time = last_update_time.lock().await;
                        let elapsed = tokio::time::Instant::now().duration_since(*last_time);
                        if elapsed.as_millis() > 0 && bytes > 0 {
                            speed = (bytes * 1000) / elapsed.as_millis() as u64;
                        } else if bytes == 0 {
                            // If there is no new data transmission, gradually reduce the speed to reflect the actual
                            // situation.
                            speed = match speed {
                                speed if speed > 1_000_000 => speed.saturating_mul(40).saturating_div(100), /* Reduce by 60% each time */
                                speed if speed > 500_000 => speed.saturating_mul(30).saturating_div(100), /* Reduce by 70% each time */
                                speed if speed > 25_000 => speed.saturating_mul(20).saturating_div(100), /* Reduce by 80% each time */
                                speed if speed > 1_000 => speed.saturating_mul(10).saturating_div(100), /* Reduce by 90% each time */
                                _ => speed.saturating_mul(5).saturating_div(100), // Reduce by 95% each time
                            };
                        }

                        *last_time = tokio::time::Instant::now();

                        stats_cache
                            .insert(
                                rule_id,
                                RuleStats {
                                    bandwidth,
                                    speed,
                                    ..prev_stats
                                },
                            )
                            .await;
                    }
                })
            };

            let client_to_server = async {
                let mut buffer = vec![0u8; buffer_size];
                let mut last_flush_time = tokio::time::Instant::now();

                loop {
                    match tokio::time::timeout(Duration::from_secs(60), client_reader.read(&mut buffer)).await {
                        Ok(Ok(0)) => break,
                        Ok(Ok(n)) => {
                            if let Err(e) = server_writer.write_all(&buffer[..n]).await {
                                warn!(parent: &span, "error writing to server: {}", e);
                                break;
                            }

                            // Update transferred byte count
                            transferred_bytes.fetch_add(n as u64, Ordering::Relaxed);

                            // Try to refresh the buffer, but with a little latency
                            if (n == buffer_size
                                || tokio::time::Instant::now().duration_since(last_flush_time)
                                    > Duration::from_millis(50))
                                && let Err(e) = server_writer.flush().await
                            {
                                warn!(parent: &span, "error flushing server writer: {}", e);
                                break;
                            }

                            last_flush_time = tokio::time::Instant::now();
                        }
                        Ok(Err(e)) => {
                            warn!(parent: &span, "error reading from client: {}", e);
                            break;
                        }
                        Err(_) => {
                            warn!(parent: &span, "client read timeout");
                            break;
                        }
                    }
                }

                let _ = server_writer.shutdown().await;
                debug!(parent: &span, "client_to_server stream closed");
            };

            let server_to_client = async {
                let mut buffer = vec![0u8; buffer_size];
                let mut last_flush_time = tokio::time::Instant::now();

                loop {
                    match tokio::time::timeout(Duration::from_secs(300), server_reader.read(&mut buffer)).await {
                        Ok(Ok(0)) => break,
                        Ok(Ok(n)) => {
                            if let Err(e) = client_writer.write_all(&buffer[..n]).await {
                                warn!(parent: &span, "error writing to client: {}", e);
                                break;
                            }

                            // Update transferred byte count
                            transferred_bytes.fetch_add(n as u64, Ordering::Relaxed);

                            // Try to refresh the buffer, but with a little latency
                            if (n == buffer_size
                                || tokio::time::Instant::now().duration_since(last_flush_time)
                                    > Duration::from_millis(50))
                                && let Err(e) = client_writer.flush().await
                            {
                                warn!(parent: &span, "error flushing client writer: {}", e);
                                break;
                            }

                            last_flush_time = tokio::time::Instant::now();
                        }
                        Ok(Err(e)) => {
                            warn!(parent: &span, "error reading from server: {}", e);
                            break;
                        }
                        Err(_) => {
                            warn!(parent: &span, "server read timeout");
                            break;
                        }
                    }
                }

                let _ = client_writer.shutdown().await;
                debug!(parent: &span, "server_to_client stream closed");
            };

            let handle = tokio::time::timeout(
                Duration::from_secs(60 * 5), // over five minutes
                async {
                    tokio::join!(client_to_server, server_to_client);
                },
            );

            tokio::select! {
                _ = async {
                    match handle.await {
                        Ok(_) => {},
                        Err(_) => {
                            debug!(parent: &span, "connection timeout");
                        }
                    }
                } => {},
                _ = stats_updater => {
                    debug!(parent: &span, "stats updater finished unexpectedly");
                }
            }

            {
                let prev_stats = stats_cache.get(&rule_id).await.unwrap_or_default();
                stats_cache
                    .insert(
                        rule_id,
                        RuleStats {
                            connections: RuleStatsConnections {
                                tcp: prev_stats.connections.tcp - 1,
                                ..prev_stats.connections
                            },
                            ..prev_stats
                        },
                    )
                    .await;
            }
        }
        Err(e) => {
            error!(parent: &span, "failed to connect to target: {}", e);
        }
    }
}
