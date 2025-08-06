use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::{net::UdpSocket, sync::Mutex, time::Instant};
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

struct UdpClient {
    sender: tokio::sync::mpsc::Sender<Vec<u8>>,
    last_active: Instant,
}

pub async fn start_udp_forward(rule: Rule, config: AgentConfig, dal: DataAccessLayer, stats_cache: StatsCache) {
    let span = info_span!("start_udp_forward", rule_id = rule.id.as_uuid().to_string());

    let rule_id = rule.id.as_uuid();
    let target_addr = rule.target.addrs[0];

    let socket = match UdpSocket::bind(rule.listen).await {
        Ok(socket) => socket,
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

    let socket_ref = socket2::SockRef::from(&socket);
    if let Err(e) = socket_ref.set_send_buffer_size(65535 * 2) {
        warn!(parent: &span, "failed to set send buffer size: {}", e);
    }
    if let Err(e) = socket_ref.set_recv_buffer_size(65535 * 2) {
        warn!(parent: &span, "failed to set receive buffer size: {}", e);
    }

    let listener = Arc::new(socket);

    debug!(parent: &span, "udp forwarding started on {}", rule.listen);

    let clients: Arc<Mutex<HashMap<SocketAddr, UdpClient>>> = Arc::new(Mutex::new(HashMap::new()));

    let transferred_bytes = Arc::new(AtomicU64::new(0));
    let last_update_time = Arc::new(Mutex::new(Instant::now()));

    let _stats_updater = {
        let transferred_bytes = transferred_bytes.clone();
        let last_update_time = last_update_time.clone();
        let stats_cache = stats_cache.clone();
        let clients = clients.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(config.stats_update_interval));

            loop {
                interval.tick().await;

                let prev_stats = stats_cache.get(&rule_id).await.unwrap_or_default();

                let bytes = transferred_bytes.swap(0, Ordering::Relaxed);

                let bandwidth = prev_stats.bandwidth + bytes;

                let mut speed = prev_stats.speed;

                let mut last_time = last_update_time.lock().await;
                let elapsed = Instant::now().duration_since(*last_time);
                if elapsed.as_millis() > 0 && bytes > 0 {
                    speed = (bytes * 1000) / elapsed.as_millis() as u64;
                } else if bytes == 0 {
                    speed = match speed {
                        speed if speed > 1_000_000 => speed.saturating_mul(40).saturating_div(100), // 降低60%
                        speed if speed > 500_000 => speed.saturating_mul(30).saturating_div(100),   // 降低70%
                        speed if speed > 25_000 => speed.saturating_mul(20).saturating_div(100),    // 降低80%
                        speed if speed > 1_000 => speed.saturating_mul(10).saturating_div(100),     // 降低90%
                        _ => speed.saturating_mul(5).saturating_div(100),                           // 降低95%
                    };
                }

                *last_time = Instant::now();

                let connection_count = clients.lock().await.len() as u64;

                stats_cache
                    .insert(
                        rule_id,
                        RuleStats {
                            bandwidth,
                            speed,
                            connections: RuleStatsConnections {
                                udp: connection_count,
                                ..prev_stats.connections
                            },
                            ..prev_stats
                        },
                    )
                    .await;
            }
        })
    };

    let _client_cleaner = {
        let clients = clients.clone();
        let span = span.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;
                let now = Instant::now();
                let mut clients_lock = clients.lock().await;

                let before_count = clients_lock.len();
                clients_lock.retain(|_, client| now.duration_since(client.last_active) < Duration::from_secs(60));
                let removed = before_count - clients_lock.len();

                if removed > 0 {
                    debug!(parent: &span, "removed {} inactive UDP clients", removed);
                }
            }
        })
    };

    let mut buf = [0; 65535];

    loop {
        let span = span.clone();

        match listener.recv_from(&mut buf).await {
            Ok((size, client_addr)) => {
                trace!(parent: &span, "received {} bytes from {}", size, client_addr);

                let data = buf[..size].to_vec();

                transferred_bytes.fetch_add(size as u64, Ordering::Relaxed);

                let mut clients_lock = clients.lock().await;

                if let Some(client) = clients_lock.get_mut(&client_addr) {
                    client.last_active = Instant::now();

                    if let Err(e) = client.sender.send(data).await {
                        error!(parent: &span, "failed to send data to client handler for {}: {}", client_addr, e);
                        clients_lock.remove(&client_addr);
                    }
                } else {
                    let listener_clone = listener.clone();
                    let (tx, rx) = tokio::sync::mpsc::channel(100);
                    let client_data = data.clone();
                    let client_transferred_bytes = transferred_bytes.clone();

                    tokio::spawn(async move {
                        match create_target_session(
                            listener_clone,
                            client_addr,
                            target_addr,
                            client_data,
                            rx,
                            client_transferred_bytes,
                        )
                        .await
                        {
                            Ok(_) => {
                                trace!(parent: &span, "udp session ended for client {}", client_addr);
                            }
                            Err(e) => {
                                warn!(parent: &span, "failed to create udp session for client {}: {}", client_addr, e);
                            }
                        }
                    });

                    clients_lock.insert(
                        client_addr,
                        UdpClient {
                            sender: tx,
                            last_active: Instant::now(),
                        },
                    );
                }
            }
            Err(e) => {
                error!(parent: &span, "failed to receive udp packet: {}", e);
            }
        }
    }
}

async fn create_target_session(
    listener: Arc<UdpSocket>, client_addr: SocketAddr, target_addr: SocketAddr, initial_data: Vec<u8>,
    mut client_rx: tokio::sync::mpsc::Receiver<Vec<u8>>, transferred_bytes: Arc<AtomicU64>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let span = info_span!(
        "udp_target_session",
        client_addr = client_addr.to_string(),
        target_addr = target_addr.to_string()
    );

    let socket = UdpSocket::bind("0.0.0.0:0").await?;

    let socket_ref = socket2::SockRef::from(&socket);
    if let Err(e) = socket_ref.set_send_buffer_size(65535 * 2) {
        debug!(parent: &span, "failed to set send buffer size: {}", e);
    }
    if let Err(e) = socket_ref.set_recv_buffer_size(65535 * 2) {
        debug!(parent: &span, "failed to set receive buffer size: {}", e);
    }

    let target_socket = Arc::new(socket);

    target_socket.as_ref().connect(target_addr).await?;

    target_socket.as_ref().send(&initial_data).await?;

    let (resp_tx, mut resp_rx) = tokio::sync::mpsc::channel(100);

    let target_socket_clone = target_socket.clone();
    let target_receiver = {
        tokio::spawn(async move {
            let mut buf = [0; 65535];
            while let Ok(size) = target_socket_clone.as_ref().recv(&mut buf).await {
                let response = buf[..size].to_vec();
                if resp_tx.send(response).await.is_err() {
                    break;
                }
            }
        })
    };

    loop {
        tokio::select! {
            Some(data) = client_rx.recv() => {
                if data.len() > 16000 {
                    trace!(parent: &span, "sending large udp packet ({} bytes) to target", data.len());

                    if let Err(e) = target_socket.as_ref().send(&data).await {
                        if let Some(code) = e.raw_os_error() {
                            if code == 40 {
                                trace!(parent: &span, "fragmenting large packet ({} bytes)", data.len());

                            let mut offset = 0;
                            let fragment_size = 8192;

                            while offset < data.len() {
                                let end = std::cmp::min(offset + fragment_size, data.len());
                                if let Err(e) = target_socket.as_ref().send(&data[offset..end]).await {
                                    error!(parent: &span, "failed to send fragment: {}", e);
                                    break;
                                }
                                offset = end;
                            }
                        } else {
                            error!(parent: &span, "failed to send data to target: {}", e);
                            break;
                        }
                        } else {
                            error!(parent: &span, "failed to send data to target: {}", e);
                            break;
                        }
                    }
                } else if let Err(e) = target_socket.as_ref().send(&data).await {
                        error!(parent: &span, "failed to send data to target: {}", e);
                        break;
                }
            }

            Some(response) = resp_rx.recv() => {
                if listener.send_to(&response, client_addr).await.is_err() {
                    break;
                }
                transferred_bytes.fetch_add(response.len() as u64, Ordering::Relaxed);
            }

            else => break,
        }
    }

    target_receiver.abort();

    Ok(())
}
