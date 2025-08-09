use std::net::SocketAddr;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleTargetPolicy {
    // If first target is unreachable, fallback to the next one, and so on
    Fallback,
    // Round-robin policy
    RoundRobin,
    // Least connections policy
    LeastConnections,
    // Random policy
    Random,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTarget {
    pub addrs: Vec<SocketAddr>,
    pub policy: RuleTargetPolicy,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleProtocol {
    Tcp,
    Udp,
    TcpUdp,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuleConfig {
    // limit maximum rule bandwidth
    pub bandwidth: Option<u64>,
    // limit maximum rule connections count
    pub connections: Option<u64>,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuleStatus {
    Running,
    #[default]
    Stopped,
    Error,
}

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuleStats {
    #[serde(rename = "rt_connections")]
    pub connections: RuleStatsConnections,
    #[serde(rename = "rt_speed")]
    pub speed: u64,
    pub bandwidth: u64,
    pub failed_times: u64,
    pub last_failed_message: String,
}

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuleStatsConnections {
    pub tcp: u64,
    pub udp: u64,
}
