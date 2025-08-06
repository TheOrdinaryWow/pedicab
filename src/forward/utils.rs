use std::sync::atomic::{AtomicUsize, Ordering};

use rand::Rng;

use crate::database::data::rule::{RuleTarget, RuleTargetPolicy};

// Helper function to select target address based on policy
pub fn select_target(target: &RuleTarget) -> std::net::SocketAddr {
    match target.policy {
        RuleTargetPolicy::Fallback => target.addrs[0], // Always use the first address
        RuleTargetPolicy::RoundRobin => {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let index = COUNTER.fetch_add(1, Ordering::Relaxed) % target.addrs.len();
            target.addrs[index]
        }
        RuleTargetPolicy::Random => {
            let mut rng = rand::rng();
            let index = rng.random_range(0..target.addrs.len());
            target.addrs[index]
        }
        RuleTargetPolicy::LeastConnections => {
            // UDP is connectionless, so fallback to Random
            let mut rng = rand::rng();
            let index = rng.random_range(0..target.addrs.len());
            target.addrs[index]
        }
    }
}
