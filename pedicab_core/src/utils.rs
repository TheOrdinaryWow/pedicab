#[cfg(unix)]
pub mod unix_limits {
    use std::{cmp, io};

    use rlimit::Resource;
    use tracing::warn;

    const DEFAULT_NOFILE_LIMIT: u64 = 1 << 16;
    const EXPANDED_NOFILE_LIMIT: u64 = 1 << 20;

    /// Try to increase NOFILE limit and return the current soft limit.
    pub fn increase_nofile_limit(expanded: bool) -> io::Result<()> {
        let (_, hard) = Resource::NOFILE.get()?;

        let target = if expanded {
            warn!("Expanded NOFILE limit is enabled: {}", EXPANDED_NOFILE_LIMIT);
            EXPANDED_NOFILE_LIMIT
        } else {
            DEFAULT_NOFILE_LIMIT
        };
        let target = cmp::min(target, hard);

        Resource::NOFILE.set(target, hard)?;

        Ok(())
    }
}
