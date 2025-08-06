use std::{hash::Hasher, net::SocketAddr};

use ahash::AHasher;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::database::data::{generic::*, rule::*};

#[derive(Debug, Encode, Decode, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Rule {
    #[serde(
        serialize_with = "serialize_compact_uuid",
        deserialize_with = "deserialize_compact_uuid"
    )]
    pub id: CompactUuid,
    pub name: String,

    pub listen: SocketAddr,
    pub target: RuleTarget,
    pub protocol: RuleProtocol,
    pub config: RuleConfig,

    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub status: RuleStatus,

    #[serde(skip)]
    pub stats: RuleStats,

    pub remarks: String,
}

fn default_enabled() -> bool {
    true
}

fn serialize_compact_uuid<S>(uuid: &CompactUuid, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    uuid.as_uuid().serialize(serializer)
}

fn deserialize_compact_uuid<'de, D>(deserializer: D) -> Result<CompactUuid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let uuid = uuid::Uuid::deserialize(deserializer)?;
    Ok(CompactUuid::from_uuid(uuid))
}

impl Rule {
    pub fn digest_config(&self) -> u64 {
        let mut fields_to_hash = Vec::new();

        bincode::encode_into_std_write(self.listen, &mut fields_to_hash, bincode::config::standard()).unwrap();
        bincode::encode_into_std_write(&self.target, &mut fields_to_hash, bincode::config::standard()).unwrap();
        bincode::encode_into_std_write(&self.protocol, &mut fields_to_hash, bincode::config::standard()).unwrap();
        bincode::encode_into_std_write(&self.config, &mut fields_to_hash, bincode::config::standard()).unwrap();

        let mut hasher = AHasher::default();
        hasher.write(&fields_to_hash);

        hasher.finish()
    }
}
