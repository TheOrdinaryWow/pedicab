use axum::{extract::State, response::IntoResponse};
use serde::Serialize;
use sysinfo::{DiskKind, DiskUsage, Disks, Networks, System};

use crate::server::{AppState, data::response::BaseResponse};

#[derive(Debug, Serialize)]
struct CpuInfo {
    count: usize,
    global_usage: f32,
    usage_per_core: Vec<f32>,
}

#[derive(Debug, Serialize)]
struct MemoryInfo {
    used: u64,
    available: u64,
    total: u64,
}

#[derive(Debug, Serialize)]
struct SwapInfo {
    used: u64,
    total: u64,
}

#[derive(Debug, Serialize)]
struct DiskInfo {
    name: String,
    kind: DiskKind,
    available: u64,
    total: u64,
    usage: DiskUsage,
}

#[derive(Debug, Serialize)]
struct SystemInfo {
    cpu: CpuInfo,
    memory: MemoryInfo,
    swap: SwapInfo,
    disk: Vec<DiskInfo>,
}

#[derive(Debug, Serialize)]
struct NetworkInfo {
    traffic_sent: u64,
    traffic_received: u64,
    // io_upload: u64,
    // io_download: u64,
    // connections_tcp: u64,
    // connections_udp: u64,
}

#[derive(Debug, Serialize)]
struct HostInfo {
    os_name: String,
    host_name: String,
    uptime: u64,
    boot_time: u64,
    cpu_arch: String,
    kernel_version: String,
}

pub async fn get_system_info() -> impl IntoResponse {
    let mut system = System::new_all();
    let disks = Disks::new_with_refreshed_list();

    // Get actual CPU usage
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    system.refresh_cpu_all();

    let system_info = SystemInfo {
        cpu: CpuInfo {
            count: system.cpus().len(),
            global_usage: system.global_cpu_usage(),
            usage_per_core: system.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(),
        },
        memory: MemoryInfo {
            used: system.used_memory(),
            available: system.available_memory(),
            total: system.total_memory(),
        },
        swap: SwapInfo {
            used: system.used_swap(),
            total: system.total_swap(),
        },
        disk: disks
            .into_iter()
            .map(|disk| DiskInfo {
                name: disk.name().to_owned().into_string().unwrap(),
                kind: disk.kind(),
                available: disk.available_space(),
                total: disk.total_space(),
                usage: disk.usage(),
            })
            .collect(),
    };

    BaseResponse::success(system_info)
}

pub async fn get_network_info(State(_state): State<AppState>) -> impl IntoResponse {
    let networks = Networks::new_with_refreshed_list();

    let non_loopback_interfaces = networks
        .into_iter()
        .filter(|&(interface_name, _)| interface_name != "lo" && interface_name != "lo0")
        .collect::<Vec<_>>();

    let network_info = NetworkInfo {
        traffic_sent: non_loopback_interfaces
            .clone()
            .into_iter()
            .map(|(_, data)| data.total_transmitted())
            .sum::<u64>(),
        traffic_received: non_loopback_interfaces
            .into_iter()
            .map(|(_, data)| data.total_received())
            .sum(),
    };

    BaseResponse::success(network_info)
}

pub async fn get_host_info() -> impl IntoResponse {
    let _ = System::new_all();

    let host_info = HostInfo {
        os_name: System::name().unwrap_or(String::from("unknown")),
        host_name: System::host_name().unwrap_or(String::from("unknown")),
        uptime: System::uptime(),
        boot_time: System::boot_time(),
        cpu_arch: System::cpu_arch(),
        kernel_version: System::kernel_version().unwrap_or(String::from("unknown")),
    };

    BaseResponse::success(host_info)
}
