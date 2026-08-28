//! Explicit bounded maintenance transitions for vector-index resources.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{
    vector_index_resource::validate_identity, EmbeddingProfile, VectorIndexHandle,
    VectorIndexHealth, VectorIndexMember, VectorIndexResourceRefusal, VectorIndexState,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorIndexLifecycle {
    Idle,
    Rebuilding {
        operation_identity: String,
        started_generation: u64,
    },
    Compacting {
        operation_identity: String,
        started_generation: u64,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorIndexMaintenanceKind {
    Rebuild,
    Compaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorIndexMaintenanceReceipt {
    pub operation_identity: String,
    pub kind: VectorIndexMaintenanceKind,
    pub prior_generation: u64,
    pub generation: u64,
    pub item_count: u32,
    pub stored_bytes: u64,
    pub completed: bool,
    pub cancelled: bool,
}

impl VectorIndexState {
    pub fn begin_maintenance(
        &mut self,
        handle: &VectorIndexHandle,
        operation_identity: String,
        kind: VectorIndexMaintenanceKind,
    ) -> Result<VectorIndexMaintenanceReceipt, VectorIndexResourceRefusal> {
        self.validate_handle(handle)?;
        self.require_maintenance_authority(handle)?;
        validate_identity(&operation_identity)?;
        let (prior_generation, generation) = self.advance_generation()?;
        self.lifecycle = match kind {
            VectorIndexMaintenanceKind::Rebuild => VectorIndexLifecycle::Rebuilding {
                operation_identity: operation_identity.clone(),
                started_generation: generation,
            },
            VectorIndexMaintenanceKind::Compaction => VectorIndexLifecycle::Compacting {
                operation_identity: operation_identity.clone(),
                started_generation: generation,
            },
        };
        Ok(self.maintenance_receipt(
            operation_identity,
            kind,
            prior_generation,
            generation,
            false,
            false,
        ))
    }

    pub fn complete_rebuild(
        &mut self,
        handle: &VectorIndexHandle,
        operation_identity: &str,
        profile: EmbeddingProfile,
        members: Vec<VectorIndexMember>,
    ) -> Result<VectorIndexMaintenanceReceipt, VectorIndexResourceRefusal> {
        self.validate_active_maintenance(
            handle,
            operation_identity,
            VectorIndexMaintenanceKind::Rebuild,
        )?;
        profile
            .validate()
            .map_err(|_| VectorIndexResourceRefusal::InvalidEmbeddingProfile)?;
        let members = self.validate_replacement_members(members)?;
        self.finish_maintenance(
            operation_identity,
            VectorIndexMaintenanceKind::Rebuild,
            Some(profile),
            members,
        )
    }

    pub fn complete_compaction(
        &mut self,
        handle: &VectorIndexHandle,
        operation_identity: &str,
        members: Vec<VectorIndexMember>,
    ) -> Result<VectorIndexMaintenanceReceipt, VectorIndexResourceRefusal> {
        self.validate_active_maintenance(
            handle,
            operation_identity,
            VectorIndexMaintenanceKind::Compaction,
        )?;
        let members = self.validate_replacement_members(members)?;
        let mut current: Vec<_> = self
            .members
            .iter()
            .map(|member| member.source_identity.as_str())
            .collect();
        let replacement: Vec<_> = members
            .iter()
            .map(|member| member.source_identity.as_str())
            .collect();
        current.sort_unstable();
        if current != replacement {
            return Err(VectorIndexResourceRefusal::SourceSetMismatch);
        }
        self.finish_maintenance(
            operation_identity,
            VectorIndexMaintenanceKind::Compaction,
            None,
            members,
        )
    }

    pub fn cancel_maintenance(
        &mut self,
        handle: &VectorIndexHandle,
        operation_identity: &str,
    ) -> Result<VectorIndexMaintenanceReceipt, VectorIndexResourceRefusal> {
        let kind = self.active_kind(operation_identity)?;
        self.validate_active_maintenance(handle, operation_identity, kind)?;
        let (prior_generation, generation) = self.advance_generation()?;
        self.lifecycle = VectorIndexLifecycle::Idle;
        Ok(self.maintenance_receipt(
            operation_identity.into(),
            kind,
            prior_generation,
            generation,
            false,
            true,
        ))
    }

    fn validate_active_maintenance(
        &self,
        handle: &VectorIndexHandle,
        operation_identity: &str,
        kind: VectorIndexMaintenanceKind,
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
        self.require_maintenance_authority(handle)?;
        if self.active_kind(operation_identity)? != kind {
            return Err(VectorIndexResourceRefusal::WrongMaintenanceOperation);
        }
        Ok(())
    }

    fn require_maintenance_authority(
        &self,
        handle: &VectorIndexHandle,
    ) -> Result<(), VectorIndexResourceRefusal> {
        if self.authority(&handle.authority_identity)?.maintain {
            Ok(())
        } else {
            Err(VectorIndexResourceRefusal::MaintenanceNotAuthorized)
        }
    }

    fn active_kind(
        &self,
        operation_identity: &str,
    ) -> Result<VectorIndexMaintenanceKind, VectorIndexResourceRefusal> {
        match &self.lifecycle {
            VectorIndexLifecycle::Rebuilding {
                operation_identity: active,
                ..
            } if active == operation_identity => Ok(VectorIndexMaintenanceKind::Rebuild),
            VectorIndexLifecycle::Compacting {
                operation_identity: active,
                ..
            } if active == operation_identity => Ok(VectorIndexMaintenanceKind::Compaction),
            _ => Err(VectorIndexResourceRefusal::WrongMaintenanceOperation),
        }
    }

    fn validate_replacement_members(
        &self,
        mut members: Vec<VectorIndexMember>,
    ) -> Result<Vec<VectorIndexMember>, VectorIndexResourceRefusal> {
        if members.len() > self.contract.bounds.maximum_items as usize {
            return Err(VectorIndexResourceRefusal::ItemLimitExceeded);
        }
        members.sort_by(|left, right| left.source_identity.cmp(&right.source_identity));
        let mut stored_bytes = 0_u64;
        for (index, member) in members.iter().enumerate() {
            validate_identity(&member.source_identity)?;
            if member.stored_bytes == 0 {
                return Err(VectorIndexResourceRefusal::StorageLimitExceeded);
            }
            if index > 0 && members[index - 1].source_identity == member.source_identity {
                return Err(VectorIndexResourceRefusal::SourceAlreadyPresent);
            }
            stored_bytes = stored_bytes
                .checked_add(member.stored_bytes)
                .ok_or(VectorIndexResourceRefusal::StorageAccountingOverflow)?;
        }
        if stored_bytes > self.contract.bounds.maximum_storage_bytes {
            return Err(VectorIndexResourceRefusal::StorageLimitExceeded);
        }
        Ok(members)
    }

    fn finish_maintenance(
        &mut self,
        operation_identity: &str,
        kind: VectorIndexMaintenanceKind,
        profile: Option<EmbeddingProfile>,
        members: Vec<VectorIndexMember>,
    ) -> Result<VectorIndexMaintenanceReceipt, VectorIndexResourceRefusal> {
        let (prior_generation, generation) = self.advance_generation()?;
        if let Some(profile) = profile {
            self.contract.embedding_profile = profile;
        }
        self.members = members;
        self.lifecycle = VectorIndexLifecycle::Idle;
        Ok(self.maintenance_receipt(
            operation_identity.into(),
            kind,
            prior_generation,
            generation,
            true,
            false,
        ))
    }

    fn advance_generation(&mut self) -> Result<(u64, u64), VectorIndexResourceRefusal> {
        let prior = self.contract.generation;
        let generation = prior
            .checked_add(1)
            .ok_or(VectorIndexResourceRefusal::GenerationExhausted)?;
        self.contract.generation = generation;
        Ok((prior, generation))
    }

    #[allow(clippy::too_many_arguments)]
    fn maintenance_receipt(
        &self,
        operation_identity: String,
        kind: VectorIndexMaintenanceKind,
        prior_generation: u64,
        generation: u64,
        completed: bool,
        cancelled: bool,
    ) -> VectorIndexMaintenanceReceipt {
        VectorIndexMaintenanceReceipt {
            operation_identity,
            kind,
            prior_generation,
            generation,
            item_count: self.members.len() as u32,
            stored_bytes: self.stored_bytes(),
            completed,
            cancelled,
        }
    }
}
