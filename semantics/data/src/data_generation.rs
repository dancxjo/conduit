//! Finite append-only conformance realization for immutable data publication.

use alloc::vec::Vec;
use conduit_core::{
    semantic_digest, BoundedResourceRef, KindId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, ValuePayload,
};

use crate::{
    data_access_class, maximum_data_reference_encoded_bytes, DataGenerationDigest,
    DataGenerationNamespace, DataGenerationNamespaceRefusal, DataGenerationRefusal, DataReference,
};

const DATA_VERSION_DIGEST_DOMAIN: &str = "data/immutable-generation-version@1";
const DATA_NAMESPACE_DIGEST_DOMAIN: &str = "data/generation-namespace@1";
pub const MAXIMUM_DATA_GENERATION_NAMESPACE_BYTES: usize = 128;

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
        let digest = semantic_digest(DATA_NAMESPACE_DIGEST_DOMAIN, semantic_identity.as_bytes());
        Self::from_digest(
            DataGenerationDigest::new(digest)
                .expect("a 32-byte digest satisfies its exact authored collection"),
        )
        .map_err(|_| DataGenerationNamespaceRefusal::TooLarge)
    }

    pub const fn digest(self) -> [u8; 32] {
        *self.identity().get()
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedGeneration {
    published: bool,
    reference: BoundedResourceRef,
    encoded: Vec<u8>,
}

/// Allocation-prepared append-only residence for one exact content Kind.
///
/// All slots and value buffers exist before Play. Publication and loading use
/// borrowed bytes and caller-prepared reference output, so neither operation
/// grows storage after preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDataGenerationStore {
    namespace: DataGenerationNamespace,
    content_kind: KindId,
    maximum_value_bytes: usize,
    maximum_total_bytes: usize,
    retained_bytes: usize,
    generation_count: usize,
    generations: Vec<PreparedGeneration>,
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
        let version_digest = generation_version(self.namespace, content_digest, sequence);
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

impl PreparedDataGenerationStore {
    pub fn new(
        namespace: DataGenerationNamespace,
        content_kind: KindId,
        maximum_generations: usize,
        maximum_value_bytes: usize,
        maximum_total_bytes: usize,
    ) -> Result<Self, DataGenerationRefusal> {
        if content_kind.as_str().is_empty()
            || maximum_generations == 0
            || maximum_value_bytes == 0
            || maximum_value_bytes > maximum_total_bytes
        {
            return Err(DataGenerationRefusal::InvalidBounds);
        }
        let mut generations = Vec::with_capacity(maximum_generations);
        for _ in 0..maximum_generations {
            generations.push(PreparedGeneration {
                published: false,
                reference: BoundedResourceRef {
                    identity: ResourceSemanticIdentity::from_digest([1; 32]),
                    content_profile: content_kind.clone(),
                    access_class: data_access_class(),
                    extent: ResourceExtent {
                        bytes: 0,
                        items: None,
                    },
                    lifetime: ResourceLifetime {
                        version: ResourceVersionIdentity::from_digest([1; 32]),
                        expires_at: None,
                    },
                },
                encoded: Vec::with_capacity(maximum_value_bytes),
            });
        }
        Ok(Self {
            namespace,
            content_kind,
            maximum_value_bytes,
            maximum_total_bytes,
            retained_bytes: 0,
            generation_count: 0,
            generations,
        })
    }

    pub fn publish_into(
        &mut self,
        value_kind: &KindId,
        encoded: &[u8],
        reference_output: &mut Vec<u8>,
    ) -> Result<(), DataGenerationRefusal> {
        if value_kind != &self.content_kind {
            return Err(DataGenerationRefusal::WrongContentKind);
        }
        if encoded.len() > self.maximum_value_bytes {
            return Err(DataGenerationRefusal::ValueTooLarge);
        }
        if self.generation_count == self.generations.len() {
            return Err(DataGenerationRefusal::GenerationCapacityExhausted);
        }
        let new_total = self
            .retained_bytes
            .checked_add(encoded.len())
            .ok_or(DataGenerationRefusal::ByteCapacityExhausted)?;
        if new_total > self.maximum_total_bytes {
            return Err(DataGenerationRefusal::ByteCapacityExhausted);
        }
        let required_reference_bytes =
            maximum_data_reference_encoded_bytes(self.content_kind.as_str())
                .ok_or(DataGenerationRefusal::InvalidBounds)?;
        if reference_output.capacity() < required_reference_bytes {
            return Err(DataGenerationRefusal::ReferenceOutputCapacity);
        }

        let content_digest = semantic_digest(self.content_kind.as_str(), encoded);
        let sequence = u64::try_from(self.generation_count)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(DataGenerationRefusal::GenerationCapacityExhausted)?;
        let generation = &mut self.generations[self.generation_count];
        generation.reference.identity = ResourceSemanticIdentity::from_digest(content_digest);
        generation.reference.extent.bytes = encoded.len() as u64;
        generation.reference.lifetime.version = ResourceVersionIdentity::from_digest(
            generation_version(self.namespace, content_digest, sequence),
        );
        generation.encoded.clear();
        generation.encoded.extend_from_slice(encoded);
        generation
            .reference
            .encode_into(reference_output)
            .map_err(|_| DataGenerationRefusal::ReferenceOutputCapacity)?;
        generation.published = true;
        self.generation_count += 1;
        self.retained_bytes = new_total;
        Ok(())
    }

    pub fn load_encoded(&self, encoded_reference: &[u8]) -> Result<&[u8], DataGenerationRefusal> {
        let reference = DataReference::validate_encoded_for(&self.content_kind, encoded_reference)
            .map_err(DataGenerationRefusal::Reference)?;
        let retained = self
            .generations
            .iter()
            .take(self.generation_count)
            .find(|candidate| {
                candidate.published
                    && candidate.reference.identity == reference.identity
                    && candidate.reference.lifetime.version == reference.version
            })
            .ok_or(DataGenerationRefusal::GenerationNotRetained)?;
        if retained.reference.extent != reference.extent
            || retained.encoded.len() as u64 != reference.extent.bytes
        {
            return Err(DataGenerationRefusal::ExtentMismatch);
        }
        Ok(retained.encoded.as_slice())
    }

    pub fn generation_count(&self) -> usize {
        self.generation_count
    }

    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    pub fn allocation_capacities(&self) -> (usize, usize) {
        (
            self.generations.capacity(),
            self.generations
                .iter()
                .map(|generation| generation.encoded.capacity())
                .sum(),
        )
    }
}

fn generation_version(
    namespace: DataGenerationNamespace,
    content_digest: [u8; 32],
    sequence: u64,
) -> [u8; 32] {
    let mut source = [0_u8; 72];
    source[..32].copy_from_slice(&namespace.digest());
    source[32..64].copy_from_slice(&content_digest);
    source[64..].copy_from_slice(&sequence.to_le_bytes());
    semantic_digest(DATA_VERSION_DIGEST_DOMAIN, &source)
}
