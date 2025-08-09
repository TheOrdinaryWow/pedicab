#![allow(unused)]

use std::net::SocketAddr;

use sled::Db;
use uuid::{NoContext, Timestamp, Uuid};

use crate::{
    data::{generic::CompactUuid, rule::*},
    model,
    model::rule::Rule,
};

#[derive(Debug, Clone)]
pub struct RuleDataAccessLayer {
    db: Db,
}

impl RuleDataAccessLayer {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}


#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Rule not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    Db(#[from] sled::Error),

    #[error("Logics error: {0}")]
    Logics(String),

    #[error("Decode error: {0}")]
    DecodeError(#[from] bincode::error::DecodeError),

    #[error("Encode error: {0}")]
    EncodeError(#[from] bincode::error::EncodeError),
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateRuleParams {
    pub name: String,
    pub listen: SocketAddr,
    pub target: RuleTarget,
    pub protocol: RuleProtocol,
    pub config: Option<RuleConfig>,
    pub enabled: Option<bool>,
    pub status: Option<RuleStatus>,
    pub remarks: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateRuleParams {
    pub name: Option<String>,
    pub listen: Option<SocketAddr>,
    pub target: Option<RuleTarget>,
    pub protocol: Option<RuleProtocol>,
    pub config: Option<RuleConfig>,
    pub enabled: Option<bool>,
    pub status: Option<RuleStatus>,
    pub remarks: Option<String>,
}

impl RuleDataAccessLayer {
    fn id_to_key(id: &Uuid) -> Vec<u8> {
        id.as_bytes().to_vec()
    }

    fn get_rule_index(&self) -> Result<Vec<Uuid>, Error> {
        let index_key = b"__rule_index".to_vec();

        if let Some(data) = self.db.get(&index_key)? {
            let ids = bincode::decode_from_slice::<Vec<CompactUuid>, _>(&data, bincode::config::standard())?;
            Ok(ids.0.iter().map(|id| id.as_uuid()).collect())
        } else {
            Ok(Vec::new())
        }
    }

    async fn update_rule_index(&self, ids: &[Uuid]) -> Result<(), Error> {
        let ids: Vec<CompactUuid> = ids.iter().map(|v| v.into()).collect();

        let index_key = b"__rule_index".to_vec();
        let buffer = bincode::encode_to_vec(&ids, bincode::config::standard())?;

        self.db.insert(index_key, buffer)?;
        self.db.flush_async().await?;
        Ok(())
    }

    async fn add_rule_id_to_index(&self, id: &Uuid) -> Result<(), Error> {
        let mut ids = self.get_rule_index()?;
        if !ids.contains(id) {
            ids.push(*id);
            self.update_rule_index(&ids).await?;
        }
        Ok(())
    }

    async fn remove_rule_id_from_index(&self, id: &Uuid) -> Result<(), Error> {
        let mut ids = self.get_rule_index()?;
        ids.retain(|&x| x != *id);
        self.update_rule_index(&ids).await?;
        Ok(())
    }
}

impl RuleDataAccessLayer {
    pub async fn find_all(&self) -> Result<Vec<Rule>, Error> {
        let ids = self.get_rule_index()?;

        let mut rules = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(rule) = self.find_by_id(id).await? {
                rules.push(rule);
            }
        }

        Ok(rules)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Rule>, Error> {
        let key = Self::id_to_key(&id);

        if let Some(data) = self.db.get(key)? {
            let rule: (Rule, usize) = bincode::decode_from_slice(&data, bincode::config::standard())?;
            Ok(Some(rule.0))
        } else {
            Ok(None)
        }
    }

    pub async fn count(&self) -> Result<u64, Error> {
        let ids = self.get_rule_index()?;
        Ok(ids.len() as u64)
    }

    pub async fn create(&self, params: CreateRuleParams) -> Result<Rule, Error> {
        let id = Uuid::new_v7(Timestamp::now(NoContext));

        let rule = Rule {
            id: id.into(),
            name: params.name,
            listen: params.listen,
            target: params.target,
            protocol: params.protocol,
            config: params.config.unwrap_or_default(),
            enabled: params.enabled.unwrap_or(false),
            status: params.status.unwrap_or_default(),
            stats: RuleStats::default(),
            remarks: params.remarks.unwrap_or(String::from("")),
        };

        let key = Self::id_to_key(&id);
        let buffer = bincode::encode_to_vec(&rule, bincode::config::standard())?;

        self.db.insert(key, buffer)?;
        self.add_rule_id_to_index(&id).await?;
        self.db.flush_async().await?;

        Ok(rule)
    }

    pub async fn update(&self, id: Uuid, params: UpdateRuleParams) -> Result<Rule, Error> {
        let key = Self::id_to_key(&id);

        let data = self.db.get(&key)?.ok_or_else(|| Error::NotFound(id.to_string()))?;
        let mut rule = bincode::decode_from_slice::<Rule, _>(&data, bincode::config::standard())?.0;

        if let Some(name) = params.name {
            rule.name = name;
        }
        if let Some(listen) = params.listen {
            rule.listen = listen;
        }
        if let Some(target) = params.target {
            rule.target = target;
        }
        if let Some(protocol) = params.protocol {
            rule.protocol = protocol;
        }
        if let Some(config) = params.config {
            rule.config = config;
        }
        if let Some(enabled) = params.enabled {
            rule.enabled = enabled;
        }
        if let Some(status) = params.status {
            rule.status = status;
        }
        if let Some(remarks) = params.remarks {
            rule.remarks = remarks;
        }

        let buffer = bincode::encode_to_vec(&rule, bincode::config::standard())?;
        self.db.insert(key, buffer)?;
        self.db.flush_async().await?;

        Ok(rule)
    }

    pub async fn update_status(&self, id: Uuid, status: RuleStatus) -> Result<(), Error> {
        let key = Self::id_to_key(&id);

        let data = self.db.get(&key)?.ok_or_else(|| Error::NotFound(id.to_string()))?;
        let mut rule = bincode::decode_from_slice::<Rule, _>(&data, bincode::config::standard())?.0;

        rule.status = status;

        let buffer = bincode::encode_to_vec(&rule, bincode::config::standard())?;

        self.db.insert(key, buffer)?;
        self.db.flush_async().await?;

        Ok(())
    }

    pub async fn update_stats(&self, id: Uuid, stats: RuleStats) -> Result<(), Error> {
        let key = Self::id_to_key(&id);

        let data = self.db.get(&key)?.ok_or_else(|| Error::NotFound(id.to_string()))?;
        let mut rule = bincode::decode_from_slice::<Rule, _>(&data, bincode::config::standard())?.0;

        rule.stats = stats;

        let buffer = bincode::encode_to_vec(&rule, bincode::config::standard())?;

        self.db.insert(key, buffer)?;
        self.db.flush_async().await?;

        Ok(())
    }

    pub async fn enable(&self, id: Uuid) -> Result<(), Error> {
        let key = Self::id_to_key(&id);

        let data = self.db.get(&key)?.ok_or_else(|| Error::NotFound(id.to_string()))?;
        let mut rule = bincode::decode_from_slice::<Rule, _>(&data, bincode::config::standard())?.0;

        rule.enabled = true;
        rule.status = RuleStatus::Stopped;

        let buffer = bincode::encode_to_vec(&rule, bincode::config::standard())?;

        self.db.insert(key, buffer)?;
        self.db.flush_async().await?;

        Ok(())
    }

    pub async fn disable(&self, id: Uuid) -> Result<(), Error> {
        let key = Self::id_to_key(&id);

        let data = self.db.get(&key)?.ok_or_else(|| Error::NotFound(id.to_string()))?;
        let mut rule = bincode::decode_from_slice::<Rule, _>(&data, bincode::config::standard())?.0;

        rule.enabled = false;
        rule.status = RuleStatus::Stopped;

        let buffer = bincode::encode_to_vec(&rule, bincode::config::standard())?;

        self.db.insert(key, buffer)?;
        self.db.flush_async().await?;

        Ok(())
    }

    pub async fn delete(&self, id: Uuid) -> Result<bool, Error> {
        let key = Self::id_to_key(&id);

        let existed = self.db.contains_key(&key)?;
        if existed {
            self.db.remove(&key)?;
            self.remove_rule_id_from_index(&id).await?;
            self.db.flush_async().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
