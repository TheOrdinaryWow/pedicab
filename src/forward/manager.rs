use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::anyhow;
use moka::future::Cache;
use tokio::{sync::RwLock, task::JoinHandle, time};
use tracing::{debug, error, info, info_span, trace, warn};
use uuid::Uuid;

use crate::{
    cli::AgentConfig,
    database::{
        dal::DataAccessLayer,
        data::rule::{RuleProtocol, RuleStats, RuleStatus},
        model::rule::Rule,
    },
    forward::{tcp::start_tcp_forward, udp::start_udp_forward},
};

pub type StatsCache = Cache<Uuid, RuleStats, ahash::RandomState>;

#[derive(Clone)]
pub struct ForwardManager {
    dal: DataAccessLayer,
    config: AgentConfig,
    stats_cache: StatsCache,
    rules: Arc<RwLock<Vec<(Uuid, u64)>>>, // [1] is rule digest
    tasks: Arc<RwLock<HashMap<Uuid, JoinHandle<()>>>>,
}

impl ForwardManager {
    pub async fn new(dal: DataAccessLayer, config: AgentConfig) -> Self {
        let manager = ForwardManager {
            dal,
            config,
            stats_cache: Cache::builder()
                .max_capacity(10_000_000)
                .build_with_hasher(ahash::RandomState::default()),
            rules: Arc::new(RwLock::new(Vec::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
        };

        info!("forward manager initiated");

        let _ = manager.load_rules().await;

        // load persistent stats from db
        {
            let persistent_stats = manager
                .dal
                .rule
                .find_all()
                .await
                .unwrap_or_else(|e| {
                    error!("error occurred loading rules: {}", e);
                    Vec::new()
                })
                .iter()
                .map(|rule| (rule.id.as_uuid(), rule.stats.clone()))
                .collect::<Vec<(_, _)>>();

            for (key, value) in persistent_stats {
                manager
                    .stats_cache
                    .insert(
                        key,
                        RuleStats {
                            bandwidth: value.bandwidth,
                            failed_times: value.failed_times,
                            // last_failed_message: value.last_failed_message,
                            ..Default::default()
                        },
                    )
                    .await;
            }
        }

        manager
    }

    async fn load_rules(&self) {
        let span = info_span!("load_rules");

        let db_rules = self.dal.rule.find_all().await.unwrap_or_else(|e| {
            error!(parent: &span, "error occurred loading rules: {}", e);
            Vec::new()
        });

        let mut current_rules = self.rules.write().await;

        // stop disabled rules
        for (rule_id, _) in current_rules.iter() {
            if !db_rules.iter().any(|r| &r.id.as_uuid() == rule_id && r.enabled) {
                let _ = self.stop_rule(*rule_id).await;
            }
        }

        for rule in db_rules.iter().filter(|rule| rule.status != RuleStatus::Error) {
            // start enabled rules
            if rule.enabled && !current_rules.iter().any(|(rule_id, _)| *rule_id == rule.id.as_uuid()) {
                let _ = self.start_rule(rule.id.as_uuid()).await;
            }

            // restart changed rules
            if let Some((rule_id, rule_digest)) = current_rules
                .iter_mut()
                .find(|(rule_id, _)| *rule_id == rule.id.as_uuid())
                && *rule_digest != rule.digest_config()
            {
                let _ = self.stop_rule(*rule_id).await;
                let _ = self.start_rule(*rule_id).await;
            }
        }

        let wanted_rules = db_rules
            .into_iter()
            .filter(|rule| rule.enabled && rule.status != RuleStatus::Error)
            .map(|rule| (rule.id.as_uuid(), rule.digest_config()))
            .collect::<Vec<(_, _)>>();

        *current_rules = wanted_rules.clone();

        let rules = wanted_rules
            .clone()
            .iter()
            .map(|(rule, _)| rule.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        trace!(parent: &span, size = wanted_rules.len(), rules, "rules loaded");
    }

    pub async fn start_polling(&self) -> anyhow::Result<()> {
        let interval_duration = if cfg!(debug_assertions) {
            Duration::from_secs(5)
        } else {
            Duration::from_secs(1)
        };
        let mut interval = time::interval(interval_duration);

        loop {
            interval.tick().await;
            self.flush_stats().await;
            self.load_rules().await;
        }
    }


    pub async fn get_rules(&self) -> Vec<Rule> {
        let lock = self.rules.read().await;
        let mut rules = Vec::new();

        for (rule_id, _) in lock.clone().into_iter() {
            if let Ok(Some(rule)) = self.dal.rule.find_by_id(rule_id).await {
                rules.push(rule);
            }
        }

        rules
    }

    pub async fn start_rule(&self, id: Uuid) -> anyhow::Result<(), anyhow::Error> {
        let span = info_span!("start_rule", id = id.to_string());

        let rule = self.dal.rule.find_by_id(id).await?.ok_or_else(|| {
            warn!(parent: &span, "rule does not exist");
            anyhow!("rule not found")
        })?;

        if !rule.enabled {
            warn!(parent: &span, "trying to start disabled rule");
            return Err(anyhow!("rule is disabled"));
        }

        let mut tasks = self.tasks.write().await;

        match rule.protocol {
            RuleProtocol::Tcp => {
                let task = tokio::spawn(start_tcp_forward(
                    rule.clone(),
                    self.config.clone(),
                    self.dal.clone(),
                    self.stats_cache.clone(),
                ));
                tasks.insert(rule.id.into(), task);
            }
            RuleProtocol::Udp => {
                let task = tokio::spawn(start_udp_forward(
                    rule.clone(),
                    self.config.clone(),
                    self.dal.clone(),
                    self.stats_cache.clone(),
                ));
                tasks.insert(rule.id.into(), task);
            }
            RuleProtocol::TcpUdp => {
                let tcp_task = tokio::spawn(start_tcp_forward(
                    rule.clone(),
                    self.config.clone(),
                    self.dal.clone(),
                    self.stats_cache.clone(),
                ));
                let udp_task = tokio::spawn(start_udp_forward(
                    rule.clone(),
                    self.config.clone(),
                    self.dal.clone(),
                    self.stats_cache.clone(),
                ));

                let tcp_udp_task = tokio::spawn(async move {
                    tokio::select! {
                        _ = tcp_task => {},
                        _ = udp_task => {},
                    }
                });

                tasks.insert(rule.id.into(), tcp_udp_task);
            }
        }

        self.dal.rule.update_status(id, RuleStatus::Running).await?;

        debug!(parent: &span, "rule started");

        Ok(())
    }

    pub async fn stop_rule(&self, id: Uuid) -> anyhow::Result<(), anyhow::Error> {
        let span = info_span!("stop_rule", id = id.to_string());

        let mut current_rules = self.rules.write().await;
        let task = self.tasks.write().await.remove(&id);

        // it has to be done anyway so it's fine
        let _ = self.dal.rule.update_status(id, RuleStatus::Stopped).await;
        if let Some(i) = current_rules.iter().position(|(rule_id, _)| *rule_id == id) {
            current_rules.remove(i);
        }

        match task {
            Some(task) => {
                debug!(parent: &span, "rule stopped");
                task.abort();
            }
            None => {
                warn!(parent: &span, "trying to stop a non-existent rule");
                return Err(anyhow!("rule not found"));
            }
        }

        Ok(())
    }

    pub async fn restart_rule(&self, id: Uuid) -> anyhow::Result<(), anyhow::Error> {
        self.stop_rule(id).await?;
        self.start_rule(id).await?;
        Ok(())
    }

    pub async fn get_stat(&self, id: Uuid) -> Option<RuleStats> {
        self.stats_cache.get(&id).await
    }

    pub async fn get_stats(&self) -> HashMap<Uuid, RuleStats> {
        let current_rules = self.rules.read().await;

        self.stats_cache
            .iter()
            .map(|(id, stat)| (*id, stat.clone()))
            .filter(|(id, _)| current_rules.iter().any(|(rule_id, _)| *rule_id == *id))
            .collect::<HashMap<Uuid, RuleStats>>()
    }

    async fn flush_stats(&self) {
        let span = info_span!("flush_stats");

        let db_rules = self.dal.rule.find_all().await.unwrap_or_else(|e| {
            error!("error occurred loading rules: {}", e);
            Vec::new()
        });

        let stats = self.get_stats().await;

        trace!(parent: &span, "function entered");

        for (id, stat) in stats {
            if db_rules.iter().any(|r| r.id.as_uuid() == id)
                && let Err(e) = self.dal.rule.update_stats(id, stat).await
            {
                debug!(
                    parent: &span,
                    "error occurred updating stats for rule {}: {}", id, e
                );
            }
        }

        trace!(parent: &span, "function ended");
    }

    pub async fn reset_stat(&self, id: Uuid) -> anyhow::Result<()> {
        let span = info_span!("reset_stat", id = id.to_string());

        if self.stats_cache.get(&id).await.is_some() {
            self.stats_cache.insert(id, RuleStats::default()).await;
            debug!(parent: &span, "rule stats reset");
            Ok(())
        } else {
            Err(anyhow::anyhow!("rule not found"))
        }
    }

    pub async fn reset_stats(&self) -> usize {
        let span = info_span!("reset_stats");

        let mut count = 0;
        let stats = self.get_stats().await;

        for (id, _) in stats {
            self.stats_cache.insert(id, RuleStats::default()).await;
            count += 1;
        }

        debug!(parent: &span, "reset stats for {} rules", count);
        count
    }
}
