use std::sync::atomic::{AtomicUsize, Ordering};

use rand::Rng;

use crate::database::data::rule::{RuleTarget, RuleTargetPolicy};

#[cfg(unix)]
pub mod unix_limits {
    use std::{cmp, io};

    use rlimit::Resource;
    use tracing::{trace, trace_span, warn};

    const DEFAULT_NOFILE_LIMIT: u64 = 1 << 16;
    const EXPANDED_NOFILE_LIMIT: u64 = 1 << 20;

    /// Try to increase NOFILE limit and return the current soft limit.
    pub fn increase_nofile_limit(expanded: bool) -> io::Result<u64> {
        let span = trace_span!("increase_nofile_limit");

        let (soft, hard) = Resource::NOFILE.get()?;
        trace!(parent: &span, soft, hard, "before increasing");

        let target = if expanded {
            warn!("Expanded NOFILE limit is enabled: {}", EXPANDED_NOFILE_LIMIT);
            EXPANDED_NOFILE_LIMIT
        } else {
            DEFAULT_NOFILE_LIMIT
        };
        let target = cmp::min(target, hard);

        trace!(parent: &span, target, "try to increase");
        Resource::NOFILE.set(target, hard)?;

        let (soft, hard) = Resource::NOFILE.get()?;
        trace!(parent: &span, soft, hard, "increasing completed");
        Ok(soft)
    }
}

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
