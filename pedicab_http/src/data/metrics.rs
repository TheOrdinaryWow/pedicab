use serde::Serialize;
use sysinfo::{DiskKind, DiskUsage, LoadAvg};

#[derive(Debug, Serialize)]
pub struct CpuInfo {
    pub count: usize,
    pub global_usage: f32,
    pub usage_per_core: Vec<f32>,
    pub load: LoadAvg,
}

#[derive(Debug, Serialize)]
pub struct MemoryInfo {
    pub used: u64,
    pub available: u64,
    pub total: u64,
}

#[derive(Debug, Serialize)]
pub struct SwapInfo {
    pub used: u64,
    pub total: u64,
}

#[derive(Debug, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub kind: DiskKind,
    pub available: u64,
    pub total: u64,
    pub usage: DiskUsage,
}

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub swap: SwapInfo,
    pub disk: Vec<DiskInfo>,
}

#[derive(Debug, Serialize)]
pub struct NetworkInfo {
    pub traffic_sent: u64,
    pub traffic_received: u64,
    // pub io_upload: u64,
    // pub io_download: u64,
    // pub connections_tcp: u64,
    // pub connections_udp: u64,
}

#[derive(Debug, Serialize)]
pub struct HostInfo {
    pub os_name: String,
    pub os_version: String,
    pub host_name: String,
    pub uptime: u64,
    pub boot_time: u64,
    pub cpu_arch: String,
    pub kernel_version: String,
}
