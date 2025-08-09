use axum::{extract::State, response::IntoResponse};
use sysinfo::{Disks, Networks, System};

use crate::{
    AppState,
    data::{metrics::*, response::BaseResponse},
};

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
            load: System::load_average(),
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
        os_version: System::long_os_version().unwrap_or(String::from("unknown")),
        host_name: System::host_name().unwrap_or(String::from("unknown")),
        uptime: System::uptime(),
        boot_time: System::boot_time(),
        cpu_arch: System::cpu_arch(),
        kernel_version: System::kernel_version().unwrap_or(String::from("unknown")),
    };

    BaseResponse::success(host_info)
}
