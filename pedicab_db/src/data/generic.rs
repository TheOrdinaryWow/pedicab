use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Encode, Decode, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactUuid(pub [u8; 16]);

impl CompactUuid {
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        CompactUuid(*uuid.as_bytes())
    }

    pub fn as_uuid(&self) -> uuid::Uuid {
        uuid::Uuid::from_bytes(self.0)
    }
}

impl From<uuid::Uuid> for CompactUuid {
    fn from(uuid: uuid::Uuid) -> Self {
        CompactUuid::from_uuid(uuid)
    }
}

impl From<&uuid::Uuid> for CompactUuid {
    fn from(uuid: &uuid::Uuid) -> Self {
        CompactUuid::from_uuid(*uuid)
    }
}

impl From<CompactUuid> for uuid::Uuid {
    fn from(uuid: CompactUuid) -> Self {
        uuid.as_uuid()
    }
}
