//! Finite append-only conformance realization for immutable data publication.

use alloc::vec::Vec;
use conduit_core::{
    semantic_digest, BoundedResourceRef, KindId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, ValuePayload,
};

use crate::{data_access_class, DataReference, DataReferenceRefusal};

const DATA_VERSION_DIGEST_DOMAIN: &str = "data/immutable-generation-version@1";
const DATA_NAMESPACE_DIGEST_DOMAIN: &str = "data/generation-namespace@1";
pub const MAXIMUM_DATA_GENERATION_NAMESPACE_BYTES: usize = 128;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DataGenerationNamespace([u8; 32]);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataGenerationNamespaceRefusal {
    Empty,
    TooLarge,
}

impl DataGenerationNamespace {
    /// Names one semantic publication scope. This identity is deliberately
    /// storage-neutral: it is not a path, pool, Host, provider, or authority.
    pub fn new(semantic_identity: &str) -> Result<Self, DataGenerationNamespaceRefusal> {
        if semantic_identity.is_empty() {
            return Err(DataGenerationNamespaceRefusal::Empty);
        }
        if semantic_identity.len() > MAXIMUM_DATA_GENERATION_NAMESPACE_BYTES {
            return Err(DataGenerationNamespaceRefusal::TooLarge);
        }
        Ok(Self(semantic_digest(
            DATA_NAMESPACE_DIGEST_DOMAIN,
            semantic_identity.as_bytes(),
        )))
    }

    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RetainedGeneration {
    reference: DataReference,
    value: ValuePayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataGenerationStore {
    namespace: DataGenerationNamespace,
    content_kind: KindId,
    maximum_generations: usize,
    maximum_total_bytes: usize,
    retained_bytes: usize,
    generations: Vec<RetainedGeneration>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataGenerationRefusal {
    InvalidBounds,
    WrongContentKind,
    ValueTooLarge,
    GenerationCapacityExhausted,
    ByteCapacityExhausted,
    Reference(DataReferenceRefusal),
    GenerationNotRetained,
    ExtentMismatch,
}

impl DataGenerationStore {
    pub fn new(
        namespace: DataGenerationNamespace,
        content_kind: KindId,
        maximum_generations: usize,
        maximum_total_bytes: usize,
    ) -> Result<Self, DataGenerationRefusal> {
        if content_kind.as_str().is_empty() || maximum_generations == 0 || maximum_total_bytes == 0
        {
            return Err(DataGenerationRefusal::InvalidBounds);
        }
        Ok(Self {
            namespace,
            content_kind,
            maximum_generations,
            maximum_total_bytes,
            retained_bytes: 0,
            generations: Vec::new(),
        })
    }

    pub fn publish(&mut self, value: ValuePayload) -> Result<DataReference, DataGenerationRefusal> {
        if value.value_kind != self.content_kind {
            return Err(DataGenerationRefusal::WrongContentKind);
        }
        let value_bytes = value.encoded.len();
        if value_bytes > self.maximum_total_bytes {
            return Err(DataGenerationRefusal::ValueTooLarge);
        }
        if self.generations.len() == self.maximum_generations {
            return Err(DataGenerationRefusal::GenerationCapacityExhausted);
        }
        let new_total = self
            .retained_bytes
            .checked_add(value_bytes)
            .ok_or(DataGenerationRefusal::ByteCapacityExhausted)?;
        if new_total > self.maximum_total_bytes {
            return Err(DataGenerationRefusal::ByteCapacityExhausted);
        }

        let content_digest = semantic_digest(self.content_kind.as_str(), &value.encoded);
        let sequence = u64::try_from(self.generations.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(DataGenerationRefusal::GenerationCapacityExhausted)?;
        let mut version_source = Vec::with_capacity(32 + content_digest.len() + 8);
        version_source.extend_from_slice(&self.namespace.digest());
        version_source.extend_from_slice(&content_digest);
        version_source.extend_from_slice(&sequence.to_le_bytes());
        let version_digest = semantic_digest(DATA_VERSION_DIGEST_DOMAIN, &version_source);
        let reference = DataReference::new(BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(content_digest),
            content_profile: self.content_kind.clone(),
            access_class: data_access_class(),
            extent: ResourceExtent {
                bytes: value_bytes as u64,
                items: None,
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest(version_digest),
                expires_at: None,
            },
        })
        .map_err(DataGenerationRefusal::Reference)?;

        self.generations.push(RetainedGeneration {
            reference: reference.clone(),
            value,
        });
        self.retained_bytes = new_total;
        Ok(reference)
    }

    pub fn load(&self, reference: &DataReference) -> Result<ValuePayload, DataGenerationRefusal> {
        reference
            .validate_for(&self.content_kind)
            .map_err(DataGenerationRefusal::Reference)?;
        let retained = self
            .generations
            .iter()
            .find(|candidate| {
                candidate.reference.reference().identity == reference.reference().identity
                    && candidate.reference.reference().lifetime.version
                        == reference.reference().lifetime.version
            })
            .ok_or(DataGenerationRefusal::GenerationNotRetained)?;
        if retained.reference.reference().extent != reference.reference().extent
            || retained.value.encoded.len() as u64 != reference.reference().extent.bytes
        {
            return Err(DataGenerationRefusal::ExtentMismatch);
        }
        Ok(retained.value.clone())
    }

    pub fn generation_count(&self) -> usize {
        self.generations.len()
    }

    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}
