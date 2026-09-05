//! Addressable Resource contracts, distinct from inline Info and Line storage.
use crate::{
    BootId, BoundedResourceRef, HostBaseId, HostId, KindId, ResourceBinding, ResourceOffer,
    ResourceRequirement,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourceRetention {
    Invocation,
    Play,
    Boot,
    BodyDurable,
    ExternalDurable,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourceSharing {
    ImmutableReadMany,
    SingleWriterPublished,
    /// Representable as a requirement, but refused by this publication profile.
    SynchronizedMutable,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourceAccessMode {
    ReadPublished,
    WriteCandidatePublish,
}

/// Finite obligations for one addressable content identity. No residence or handle.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceContentRequirement {
    pub identity: crate::ResourceSemanticIdentity,
    pub version: crate::ResourceVersionIdentity,
    pub content_profile: KindId,
    pub maximum_bytes: u32,
    pub maximum_items: u32,
    pub retention: ResourceRetention,
    pub sharing: ResourceSharing,
    pub access: ResourceAccessMode,
    pub generation_slots: u16,
    pub reader_leases: u16,
    /// The finite candidate/publication operation slot count (zero for readers).
    pub publication_slots: u16,
    pub sensitive: bool,
}

/// Implementation-owned residence facts, sealed alongside the semantic contract.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceContentOffer {
    pub contract: ResourceContentRequirement,
    pub owner_host: HostId,
    pub owner_boot: BootId,
    pub base_id: HostBaseId,
    pub residence_profile: KindId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResourceContentRefusal {
    InvalidContract,
    MultipleWriters,
    UnsupportedCoherence,
    ContractMismatch,
    ForeignResidence,
    ReferenceMismatch,
}

impl ResourceContentRequirement {
    pub fn validate(&self) -> Result<(), ResourceContentRefusal> {
        if self.sharing == ResourceSharing::SynchronizedMutable {
            return Err(ResourceContentRefusal::UnsupportedCoherence);
        }
        if self.identity.digest() == [0; 32]
            || self.version.digest() == [0; 32]
            || self.content_profile.as_str().is_empty()
            || self.content_profile.as_str().len() > 128
            || self.maximum_bytes == 0
            || self.maximum_items == 0
            || self.generation_slots == 0
            || self.reader_leases == 0
            || self.reader_leases == u16::MAX
            || self
                .maximum_bytes
                .checked_mul(u32::from(self.generation_slots))
                .is_none()
            || match self.access {
                ResourceAccessMode::ReadPublished => self.publication_slots != 0,
                ResourceAccessMode::WriteCandidatePublish => {
                    self.publication_slots != 1
                        || self.sharing != ResourceSharing::SingleWriterPublished
                }
            }
        {
            return Err(ResourceContentRefusal::InvalidContract);
        }
        Ok(())
    }

    pub fn accepts_reference(
        &self,
        reference: &BoundedResourceRef,
    ) -> Result<(), ResourceContentRefusal> {
        self.validate()?;
        if reference.validate().is_err()
            || reference.identity != self.identity
            || reference.lifetime.version != self.version
            || reference.content_profile != self.content_profile
            || reference.extent.bytes > u64::from(self.maximum_bytes)
            || reference
                .extent
                .items
                .is_none_or(|items| items > u64::from(self.maximum_items))
        {
            return Err(ResourceContentRefusal::ReferenceMismatch);
        }
        Ok(())
    }
}

impl ResourceContentOffer {
    pub fn validate(&self) -> Result<(), ResourceContentRefusal> {
        self.contract.validate()?;
        if [
            self.owner_host.as_str(),
            self.owner_boot.as_str(),
            self.base_id.as_str(),
            self.residence_profile.as_str(),
        ]
        .iter()
        .any(|id| id.is_empty() || id.len() > 128)
        {
            return Err(ResourceContentRefusal::InvalidContract);
        }
        Ok(())
    }
}

/// Exact matching deliberately refuses unsupported coherence or remote residence.
/// A remote copy/dereference implementation must advertise its own legal contract.
pub fn bind_resource_content(
    requirement: &ResourceRequirement,
    offer: &ResourceOffer,
    host: &HostId,
    boot: &BootId,
) -> Result<Option<ResourceContentOffer>, ResourceContentRefusal> {
    match (&requirement.content, &offer.content) {
        (None, None) => Ok(None),
        (Some(required), Some(offered)) => {
            required.validate()?;
            offered.validate()?;
            if !supports(required, &offered.contract)
                || requirement.compute.is_some()
                || offer.compute.is_some()
            {
                return Err(ResourceContentRefusal::ContractMismatch);
            }
            if &offered.owner_host != host || &offered.owner_boot != boot {
                return Err(ResourceContentRefusal::ForeignResidence);
            }
            let mut bound = offered.clone();
            bound.contract = required.clone();
            Ok(Some(bound))
        }
        _ => Err(ResourceContentRefusal::ContractMismatch),
    }
}

pub(crate) fn content_binding_satisfies(
    binding: &ResourceBinding,
    requirement: &ResourceRequirement,
    offer: &ResourceOffer,
) -> bool {
    match (&binding.content, &requirement.content, &offer.content) {
        (None, None, None) => true,
        (Some(bound), Some(required), Some(offered)) => {
            bound.owner_host == offered.owner_host
                && bound.owner_boot == offered.owner_boot
                && bound.base_id == offered.base_id
                && bound.residence_profile == offered.residence_profile
                && supports(required, &offered.contract)
                && &bound.contract == required
                && bound.validate().is_ok()
                && requirement.compute.is_none()
                && binding.compute.is_none()
                && offer.compute.is_none()
        }
        _ => false,
    }
}

fn supports(required: &ResourceContentRequirement, offered: &ResourceContentRequirement) -> bool {
    let mut selected = offered.clone();
    if required.access == ResourceAccessMode::ReadPublished {
        selected.access = ResourceAccessMode::ReadPublished;
        selected.publication_slots = 0;
    }
    required == &selected
}
