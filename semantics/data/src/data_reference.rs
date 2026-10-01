//! Ordinary typed references to exact immutable data generations.

use alloc::vec::Vec;
use conduit_core::{
    data_reference_kind, BoundedResourceRef, EncodedResourceReference, KindId, ResourceClassId,
    ResourceReferenceRefusal, ValuePayload, MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES,
    RESOURCE_REFERENCE_DIGEST_BYTES,
};

use crate::DataReferenceRefusal;

pub const DATA_GENERATION_ACCESS_CLASS: &str = "data/immutable-generation@1";
/// Fixed fields in an immutable, non-expiring data reference, excluding the
/// content-Kind and access-class identity bytes themselves.
pub const DATA_REFERENCE_ENCODING_OVERHEAD_BYTES: usize =
    1 + (2 * RESOURCE_REFERENCE_DIGEST_BYTES) + 2 + 2 + 8 + 1 + 1;
/// Maximum encoded size for any valid immutable data-generation reference.
pub const MAXIMUM_DATA_REFERENCE_ENCODED_BYTES: usize = DATA_REFERENCE_ENCODING_OVERHEAD_BYTES
    + MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES
    + DATA_GENERATION_ACCESS_CLASS.len();

pub const fn maximum_data_reference_encoded_bytes(content_kind: &str) -> Option<usize> {
    if content_kind.is_empty() || content_kind.len() > MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES {
        None
    } else {
        Some(
            DATA_REFERENCE_ENCODING_OVERHEAD_BYTES
                + content_kind.len()
                + DATA_GENERATION_ACCESS_CLASS.len(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataReference {
    reference: BoundedResourceRef,
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

    /// Validate and inspect one encoded data reference without allocating.
    pub fn validate_encoded_for<'a>(
        content_kind: &KindId,
        encoded: &'a [u8],
    ) -> Result<EncodedResourceReference<'a>, DataReferenceRefusal> {
        let reference =
            BoundedResourceRef::validate_encoded(encoded).map_err(map_reference_refusal)?;
        if reference.content_profile != content_kind.as_str() {
            return Err(DataReferenceRefusal::WrongContentKind);
        }
        if reference.access_class != DATA_GENERATION_ACCESS_CLASS {
            return Err(DataReferenceRefusal::WrongAccessClass);
        }
        if reference.has_expiry {
            return Err(DataReferenceRefusal::ExpiringGeneration);
        }
        if reference.extent.items.is_some() {
            return Err(DataReferenceRefusal::ItemExtent);
        }
        Ok(reference)
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

    pub fn encode_into(&self, encoded: &mut Vec<u8>) -> Result<(), DataReferenceRefusal> {
        self.reference
            .encode_into(encoded)
            .map_err(map_reference_refusal)
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
