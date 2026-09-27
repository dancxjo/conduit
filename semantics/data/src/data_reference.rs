//! Ordinary typed references to exact immutable data generations.

use alloc::vec::Vec;
use conduit_core::{
    data_reference_kind, BoundedResourceRef, KindId, ResourceClassId, ResourceReferenceRefusal,
    ValuePayload,
};

pub const DATA_GENERATION_ACCESS_CLASS: &str = "data/immutable-generation@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataReference {
    reference: BoundedResourceRef,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataReferenceRefusal {
    Malformed,
    WrongReferenceKind,
    WrongContentKind,
    WrongAccessClass,
    ExpiringGeneration,
    ItemExtent,
}

impl DataReference {
    pub fn new(reference: BoundedResourceRef) -> Result<Self, DataReferenceRefusal> {
        let value = Self { reference };
        value.validate_for(value.content_kind())?;
        Ok(value)
    }

    pub fn decode_for(content_kind: &KindId, encoded: &[u8]) -> Result<Self, DataReferenceRefusal> {
        let reference = BoundedResourceRef::decode(encoded).map_err(map_reference_refusal)?;
        let value = Self { reference };
        value.validate_for(content_kind)?;
        Ok(value)
    }

    pub fn from_value_for(
        content_kind: &KindId,
        value: &ValuePayload,
    ) -> Result<Self, DataReferenceRefusal> {
        if value.value_kind != data_reference_kind(content_kind) {
            return Err(DataReferenceRefusal::WrongReferenceKind);
        }
        Self::decode_for(content_kind, &value.encoded)
    }

    pub fn validate_for(&self, content_kind: &KindId) -> Result<(), DataReferenceRefusal> {
        self.reference.validate().map_err(map_reference_refusal)?;
        if &self.reference.content_profile != content_kind {
            return Err(DataReferenceRefusal::WrongContentKind);
        }
        if self.reference.access_class.as_str() != DATA_GENERATION_ACCESS_CLASS {
            return Err(DataReferenceRefusal::WrongAccessClass);
        }
        if self.reference.lifetime.expires_at.is_some() {
            return Err(DataReferenceRefusal::ExpiringGeneration);
        }
        if self.reference.extent.items.is_some() {
            return Err(DataReferenceRefusal::ItemExtent);
        }
        Ok(())
    }

    pub fn content_kind(&self) -> &KindId {
        &self.reference.content_profile
    }

    pub fn reference(&self) -> &BoundedResourceRef {
        &self.reference
    }

    pub fn encode(&self) -> Result<Vec<u8>, DataReferenceRefusal> {
        self.reference.encode().map_err(map_reference_refusal)
    }

    pub fn to_value(&self) -> Result<ValuePayload, DataReferenceRefusal> {
        Ok(ValuePayload {
            value_kind: data_reference_kind(self.content_kind()),
            encoded: self.encode()?,
        })
    }
}

pub(crate) fn data_access_class() -> ResourceClassId {
    ResourceClassId::from(DATA_GENERATION_ACCESS_CLASS)
}

fn map_reference_refusal(_: ResourceReferenceRefusal) -> DataReferenceRefusal {
    DataReferenceRefusal::Malformed
}
