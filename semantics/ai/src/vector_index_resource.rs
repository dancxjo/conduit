//! Provider-neutral bounded lifecycle for one exact vector-index resource.

use alloc::{string::String, vec::Vec};
use conduit_core::{
    resource_binding_satisfies, ResourceBinding, ResourceClassId, ResourceOffer, ResourcePoolId,
    ResourceRequirement,
};
use serde::{Deserialize, Serialize};

use crate::{EmbeddingProfile, VectorRefusal, MAXIMUM_SIMILARITY_TOP_K};

pub const VECTOR_INDEX_RESOURCE_CLASS: &str = "resource/vector-index@1";
pub const MAXIMUM_VECTOR_INDEX_MEMBERS: u32 = 4_096;
pub const MAXIMUM_VECTOR_INDEX_STORAGE_BYTES: u64 = 1 << 30;
pub const MAXIMUM_VECTOR_INDEX_QUERY_WORK_UNITS: u32 = 65_536;
pub const MAXIMUM_VECTOR_INDEX_CONCURRENT_QUERIES: u32 = 256;
pub const MAXIMUM_VECTOR_INDEX_AUTHORITIES: usize = 16;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorIndexAuthority {
    pub query: bool,
    pub insert: bool,
    pub upsert: bool,
    pub delete: bool,
    pub maintain: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorIndexBounds {
    pub maximum_items: u32,
    pub maximum_storage_bytes: u64,
    pub maximum_query_work_units: u32,
    pub maximum_results: u32,
    pub maximum_concurrent_queries: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorIndexContract {
    pub index_identity: String,
    pub generation: u64,
    pub embedding_profile: EmbeddingProfile,
    pub pool_id: ResourcePoolId,
    pub class_id: ResourceClassId,
    pub bounds: VectorIndexBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorIndexHandle {
    pub index_identity: String,
    pub generation: u64,
    pub authority_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorIndexAuthorization {
    pub authority_identity: String,
    pub authority: VectorIndexAuthority,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorIndexHealth {
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorIndexMember {
    pub source_identity: String,
    pub stored_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorIndexState {
    pub contract: VectorIndexContract,
    pub health: VectorIndexHealth,
    pub lifecycle: crate::VectorIndexLifecycle,
    pub(crate) authorities: Vec<VectorIndexAuthorization>,
    pub(crate) members: Vec<VectorIndexMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorIndexMutation {
    Insert {
        mutation_identity: String,
        source_identity: String,
        stored_bytes: u64,
    },
    Upsert {
        mutation_identity: String,
        source_identity: String,
        stored_bytes: u64,
    },
    Delete {
        mutation_identity: String,
        source_identity: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorIndexMutationReceipt {
    pub mutation_identity: String,
    pub source_identity: String,
    pub prior_generation: u64,
    pub generation: u64,
    pub item_count: u32,
    pub stored_bytes: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorIndexQueryAdmission {
    pub work_units: u32,
    pub maximum_results: u32,
    pub concurrent_queries: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorIndexResourceRefusal {
    InvalidIdentity,
    InvalidEmbeddingProfile,
    InvalidResourceClass,
    InvalidBounds,
    WrongIndex,
    UnknownAuthority,
    StaleGeneration,
    GenerationExhausted,
    ResourceUnavailable,
    QueryNotAuthorized,
    InsertNotAuthorized,
    UpsertNotAuthorized,
    DeleteNotAuthorized,
    MaintenanceNotAuthorized,
    ResourceBusy,
    WrongMaintenanceOperation,
    SourceSetMismatch,
    SourceAlreadyPresent,
    SourceNotPresent,
    ItemLimitExceeded,
    StorageLimitExceeded,
    StorageAccountingOverflow,
    QueryWorkLimitExceeded,
    ResultLimitExceeded,
    QueryConcurrencyExceeded,
    InvalidResourceBinding,
}

impl VectorIndexContract {
    pub fn validate(&self) -> Result<(), VectorIndexResourceRefusal> {
        validate_identity(&self.index_identity)?;
        self.embedding_profile
            .validate()
            .map_err(|_: VectorRefusal| VectorIndexResourceRefusal::InvalidEmbeddingProfile)?;
        if self.pool_id.as_str().is_empty() || self.class_id.as_str() != VECTOR_INDEX_RESOURCE_CLASS
        {
            return Err(VectorIndexResourceRefusal::InvalidResourceClass);
        }
        let bounds = self.bounds;
        if bounds.maximum_items == 0
            || bounds.maximum_items > MAXIMUM_VECTOR_INDEX_MEMBERS
            || bounds.maximum_storage_bytes == 0
            || bounds.maximum_storage_bytes > MAXIMUM_VECTOR_INDEX_STORAGE_BYTES
            || bounds.maximum_query_work_units == 0
            || bounds.maximum_query_work_units > MAXIMUM_VECTOR_INDEX_QUERY_WORK_UNITS
            || bounds.maximum_results == 0
            || bounds.maximum_results > MAXIMUM_SIMILARITY_TOP_K
            || bounds.maximum_concurrent_queries == 0
            || bounds.maximum_concurrent_queries > MAXIMUM_VECTOR_INDEX_CONCURRENT_QUERIES
        {
            return Err(VectorIndexResourceRefusal::InvalidBounds);
        }
        Ok(())
    }

    pub fn planning_offer(&self) -> Result<ResourceOffer, VectorIndexResourceRefusal> {
        self.validate()?;
        Ok(ResourceOffer {
            content: None,
            pool_id: self.pool_id.clone(),
            class_id: self.class_id.clone(),
            capacity_units: self.bounds.maximum_query_work_units,
            compute: None,
        })
    }
}

impl VectorIndexState {
    pub fn new(
        contract: VectorIndexContract,
        authorities: Vec<VectorIndexAuthorization>,
    ) -> Result<Self, VectorIndexResourceRefusal> {
        contract.validate()?;
        if authorities.is_empty()
            || authorities.len() > MAXIMUM_VECTOR_INDEX_AUTHORITIES
            || authorities
                .iter()
                .any(|entry| validate_identity(&entry.authority_identity).is_err())
            || authorities.iter().enumerate().any(|(index, entry)| {
                authorities[index + 1..]
                    .iter()
                    .any(|candidate| candidate.authority_identity == entry.authority_identity)
            })
        {
            return Err(VectorIndexResourceRefusal::InvalidIdentity);
        }
        Ok(Self {
            contract,
            health: VectorIndexHealth::Ready,
            lifecycle: crate::VectorIndexLifecycle::Idle,
            authorities,
            members: Vec::new(),
        })
    }

    pub fn members(&self) -> &[VectorIndexMember] {
        &self.members
    }

    pub fn stored_bytes(&self) -> u64 {
        self.members.iter().map(|member| member.stored_bytes).sum()
    }

    pub fn handle(
        &self,
        authority_identity: &str,
    ) -> Result<VectorIndexHandle, VectorIndexResourceRefusal> {
        self.authority(authority_identity)?;
        Ok(VectorIndexHandle {
            index_identity: self.contract.index_identity.clone(),
            generation: self.contract.generation,
            authority_identity: authority_identity.into(),
        })
    }

    pub fn admit_query(
        &self,
        handle: &VectorIndexHandle,
        admission: VectorIndexQueryAdmission,
        binding: &ResourceBinding,
    ) -> Result<(), VectorIndexResourceRefusal> {
        self.validate_handle(handle)?;
        if !self.authority(&handle.authority_identity)?.query {
            return Err(VectorIndexResourceRefusal::QueryNotAuthorized);
        }
        if admission.work_units == 0
            || admission.work_units > self.contract.bounds.maximum_query_work_units
        {
            return Err(VectorIndexResourceRefusal::QueryWorkLimitExceeded);
        }
        if admission.maximum_results == 0
            || admission.maximum_results > self.contract.bounds.maximum_results
        {
            return Err(VectorIndexResourceRefusal::ResultLimitExceeded);
        }
        if admission.concurrent_queries == 0
            || admission.concurrent_queries > self.contract.bounds.maximum_concurrent_queries
        {
            return Err(VectorIndexResourceRefusal::QueryConcurrencyExceeded);
        }
        let requirement = ResourceRequirement {
            content: None,
            class_id: self.contract.class_id.clone(),
            units: admission.work_units,
            protected_role: None,
            compute: None,
        };
        let offer = self.contract.planning_offer()?;
        if !resource_binding_satisfies(binding, &requirement, &offer) {
            return Err(VectorIndexResourceRefusal::InvalidResourceBinding);
        }
        Ok(())
    }

    pub fn mutate(
        &mut self,
        handle: &VectorIndexHandle,
        mutation: VectorIndexMutation,
    ) -> Result<VectorIndexMutationReceipt, VectorIndexResourceRefusal> {
        self.validate_handle(handle)?;
        let authority = self.authority(&handle.authority_identity)?;
        let next_generation = self
            .contract
            .generation
            .checked_add(1)
            .ok_or(VectorIndexResourceRefusal::GenerationExhausted)?;
        let (mutation_identity, source_identity, operation) = match mutation {
            VectorIndexMutation::Insert {
                mutation_identity,
                source_identity,
                stored_bytes,
            } => {
                if !authority.insert {
                    return Err(VectorIndexResourceRefusal::InsertNotAuthorized);
                }
                (
                    mutation_identity,
                    source_identity,
                    MutationOperation::Insert(stored_bytes),
                )
            }
            VectorIndexMutation::Upsert {
                mutation_identity,
                source_identity,
                stored_bytes,
            } => {
                if !authority.upsert {
                    return Err(VectorIndexResourceRefusal::UpsertNotAuthorized);
                }
                (
                    mutation_identity,
                    source_identity,
                    MutationOperation::Upsert(stored_bytes),
                )
            }
            VectorIndexMutation::Delete {
                mutation_identity,
                source_identity,
            } => {
                if !authority.delete {
                    return Err(VectorIndexResourceRefusal::DeleteNotAuthorized);
                }
                (
                    mutation_identity,
                    source_identity,
                    MutationOperation::Delete,
                )
            }
        };
        validate_identity(&mutation_identity)?;
        validate_identity(&source_identity)?;
        let existing = self
            .members
            .iter()
            .position(|member| member.source_identity == source_identity);
        match operation {
            MutationOperation::Insert(stored_bytes) => {
                if existing.is_some() {
                    return Err(VectorIndexResourceRefusal::SourceAlreadyPresent);
                }
                if self.members.len() >= self.contract.bounds.maximum_items as usize {
                    return Err(VectorIndexResourceRefusal::ItemLimitExceeded);
                }
                self.admit_storage_change(0, stored_bytes)?;
                self.members.push(VectorIndexMember {
                    source_identity: source_identity.clone(),
                    stored_bytes,
                });
            }
            MutationOperation::Upsert(stored_bytes) => {
                let index = existing.ok_or(VectorIndexResourceRefusal::SourceNotPresent)?;
                self.admit_storage_change(self.members[index].stored_bytes, stored_bytes)?;
                self.members[index].stored_bytes = stored_bytes;
            }
            MutationOperation::Delete => {
                let index = existing.ok_or(VectorIndexResourceRefusal::SourceNotPresent)?;
                self.members.remove(index);
            }
        }
        self.members
            .sort_by(|left, right| left.source_identity.cmp(&right.source_identity));
        let prior_generation = self.contract.generation;
        self.contract.generation = next_generation;
        Ok(VectorIndexMutationReceipt {
            mutation_identity,
            source_identity,
            prior_generation,
            generation: self.contract.generation,
            item_count: self.members.len() as u32,
            stored_bytes: self.stored_bytes(),
        })
    }

    pub fn mark_unavailable(
        &mut self,
        handle: &VectorIndexHandle,
    ) -> Result<u64, VectorIndexResourceRefusal> {
        self.validate_handle(handle)?;
        self.contract.generation = self
            .contract
            .generation
            .checked_add(1)
            .ok_or(VectorIndexResourceRefusal::GenerationExhausted)?;
        self.health = VectorIndexHealth::Unavailable;
        Ok(self.contract.generation)
    }

    pub(crate) fn authority(
        &self,
        authority_identity: &str,
    ) -> Result<VectorIndexAuthority, VectorIndexResourceRefusal> {
        self.authorities
            .iter()
            .find(|entry| entry.authority_identity == authority_identity)
            .map(|entry| entry.authority)
            .ok_or(VectorIndexResourceRefusal::UnknownAuthority)
    }

    pub(crate) fn validate_handle(
        &self,
        handle: &VectorIndexHandle,
    ) -> Result<(), VectorIndexResourceRefusal> {
        if handle.index_identity != self.contract.index_identity {
            return Err(VectorIndexResourceRefusal::WrongIndex);
        }
        if handle.generation != self.contract.generation {
            return Err(VectorIndexResourceRefusal::StaleGeneration);
        }
        if self.health != VectorIndexHealth::Ready {
            return Err(VectorIndexResourceRefusal::ResourceUnavailable);
        }
        if self.lifecycle != crate::VectorIndexLifecycle::Idle {
            return Err(VectorIndexResourceRefusal::ResourceBusy);
        }
        Ok(())
    }

    fn admit_storage_change(
        &self,
        prior_bytes: u64,
        replacement_bytes: u64,
    ) -> Result<(), VectorIndexResourceRefusal> {
        if replacement_bytes == 0 {
            return Err(VectorIndexResourceRefusal::StorageLimitExceeded);
        }
        let after_removal = self
            .stored_bytes()
            .checked_sub(prior_bytes)
            .ok_or(VectorIndexResourceRefusal::StorageAccountingOverflow)?;
        let next = after_removal
            .checked_add(replacement_bytes)
            .ok_or(VectorIndexResourceRefusal::StorageAccountingOverflow)?;
        if next > self.contract.bounds.maximum_storage_bytes {
            return Err(VectorIndexResourceRefusal::StorageLimitExceeded);
        }
        Ok(())
    }
}

enum MutationOperation {
    Insert(u64),
    Upsert(u64),
    Delete,
}

pub(crate) fn validate_identity(identity: &str) -> Result<(), VectorIndexResourceRefusal> {
    if identity.is_empty() || identity.len() > crate::MAXIMUM_VECTOR_IDENTITY_BYTES {
        Err(VectorIndexResourceRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
}
