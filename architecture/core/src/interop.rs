//! Bounded interoperability membrane: integration does not imply assimilation.
//!
//! External identity, semantic meaning, mapping, Base, authority, and outward
//! manifestation remain distinct. Discovery is observation only and never
//! fabricates a Host, Part, capability, trust, or authority grant.

use crate::{
    AuthorityGrantId, BaseInstanceId, ExternalManifestationId, ExternalResourceId,
    InteropAdapterId, InteropMappingId, KindContractRevision, KindId,
};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteropDirection {
    ExternalToConduit,
    ConduitToExternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExternalDeliveryContract {
    BestEffort,
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteropMapping {
    pub mapping_id: InteropMappingId,
    pub adapter_id: InteropAdapterId,
    pub base_instance_id: BaseInstanceId,
    pub external_resource_id: ExternalResourceId,
    pub semantic_kind: KindId,
    pub semantic_revision: KindContractRevision,
    pub direction: InteropDirection,
    pub authority_grant_id: AuthorityGrantId,
    pub maximum_payload_bytes: u32,
    pub maximum_queued_items: u16,
    pub external_delivery: ExternalDeliveryContract,
    /// Must equal the observed external contract. A stronger semantic promise
    /// requires a different, independently proved adapter contract.
    pub declared_semantic_delivery: ExternalDeliveryContract,
    pub preserves_external_lifecycle: bool,
    /// A reflected manifestation is imported only by an exact fresh mapping
    /// naming the export mapping that created it.
    pub intentional_reimport_of: Option<InteropMappingId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalOrigin {
    pub adapter_id: InteropAdapterId,
    pub export_mapping_id: InteropMappingId,
    pub manifestation_id: ExternalManifestationId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalObservation {
    pub adapter_id: InteropAdapterId,
    pub external_resource_id: ExternalResourceId,
    pub payload_bytes: u32,
    pub origin: Option<ExternalOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutwardManifestation {
    pub external_resource_id: ExternalResourceId,
    pub semantic_kind: KindId,
    pub origin: ExternalOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportDecision {
    Admitted {
        mapping_id: InteropMappingId,
        authority_grant_id: AuthorityGrantId,
    },
    Unconfigured,
    ReflectedManifestation,
    PayloadOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteropMembraneLimits {
    pub maximum_mappings: u16,
    pub maximum_manifestations: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteropRefusal {
    InvalidLimits,
    EmptyIdentity,
    InvalidBounds,
    MappingCapacity,
    ManifestationCapacity,
    DuplicateMapping,
    DuplicateDirectionalResource,
    UnknownMapping,
    WrongDirection,
    FabricatedGuarantee,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteropMembrane {
    limits: InteropMembraneLimits,
    mappings: Vec<InteropMapping>,
    manifestations: Vec<OutwardManifestation>,
    next_manifestation: u64,
}

impl InteropMembrane {
    pub fn new(limits: InteropMembraneLimits) -> Result<Self, InteropRefusal> {
        if limits.maximum_mappings == 0 || limits.maximum_manifestations == 0 {
            return Err(InteropRefusal::InvalidLimits);
        }
        Ok(Self {
            limits,
            mappings: Vec::with_capacity(limits.maximum_mappings.into()),
            manifestations: Vec::with_capacity(limits.maximum_manifestations.into()),
            next_manifestation: 0,
        })
    }

    pub fn mappings(&self) -> &[InteropMapping] {
        &self.mappings
    }

    pub fn manifestations(&self) -> &[OutwardManifestation] {
        &self.manifestations
    }

    pub fn register_mapping(&mut self, mapping: InteropMapping) -> Result<(), InteropRefusal> {
        validate_mapping(&mapping)?;
        if self.mappings.len() == usize::from(self.limits.maximum_mappings) {
            return Err(InteropRefusal::MappingCapacity);
        }
        if self
            .mappings
            .iter()
            .any(|existing| existing.mapping_id == mapping.mapping_id)
        {
            return Err(InteropRefusal::DuplicateMapping);
        }
        if self.mappings.iter().any(|existing| {
            existing.adapter_id == mapping.adapter_id
                && existing.external_resource_id == mapping.external_resource_id
                && existing.direction == mapping.direction
        }) {
            return Err(InteropRefusal::DuplicateDirectionalResource);
        }
        self.mappings.push(mapping);
        Ok(())
    }

    pub fn manifest(
        &mut self,
        mapping_id: &InteropMappingId,
    ) -> Result<OutwardManifestation, InteropRefusal> {
        if self.manifestations.len() == usize::from(self.limits.maximum_manifestations) {
            return Err(InteropRefusal::ManifestationCapacity);
        }
        let mapping = self
            .mappings
            .iter()
            .find(|mapping| &mapping.mapping_id == mapping_id)
            .ok_or(InteropRefusal::UnknownMapping)?;
        if mapping.direction != InteropDirection::ConduitToExternal {
            return Err(InteropRefusal::WrongDirection);
        }
        let manifestation = OutwardManifestation {
            external_resource_id: mapping.external_resource_id.clone(),
            semantic_kind: mapping.semantic_kind.clone(),
            origin: ExternalOrigin {
                adapter_id: mapping.adapter_id.clone(),
                export_mapping_id: mapping.mapping_id.clone(),
                manifestation_id: ExternalManifestationId::from(alloc::format!(
                    "manifestation/{}",
                    self.next_manifestation
                )),
            },
        };
        self.next_manifestation = self
            .next_manifestation
            .checked_add(1)
            .ok_or(InteropRefusal::ManifestationCapacity)?;
        self.manifestations.push(manifestation.clone());
        Ok(manifestation)
    }

    pub fn consider_import(&self, observation: &ExternalObservation) -> ImportDecision {
        let mapping = self.mappings.iter().find(|mapping| {
            mapping.direction == InteropDirection::ExternalToConduit
                && mapping.adapter_id == observation.adapter_id
                && mapping.external_resource_id == observation.external_resource_id
        });
        if let Some(origin) = &observation.origin {
            let is_own_manifestation = self.manifestations.iter().any(|manifestation| {
                manifestation.external_resource_id == observation.external_resource_id
                    && manifestation.origin == *origin
            });
            if is_own_manifestation
                && mapping.and_then(|mapping| mapping.intentional_reimport_of.as_ref())
                    != Some(&origin.export_mapping_id)
            {
                return ImportDecision::ReflectedManifestation;
            }
        }
        let Some(mapping) = mapping else {
            return ImportDecision::Unconfigured;
        };
        if observation.payload_bytes > mapping.maximum_payload_bytes {
            return ImportDecision::PayloadOverflow;
        }
        ImportDecision::Admitted {
            mapping_id: mapping.mapping_id.clone(),
            authority_grant_id: mapping.authority_grant_id.clone(),
        }
    }
}

fn validate_mapping(mapping: &InteropMapping) -> Result<(), InteropRefusal> {
    if mapping.mapping_id.as_str().is_empty()
        || mapping.adapter_id.as_str().is_empty()
        || mapping.base_instance_id.as_str().is_empty()
        || mapping.external_resource_id.as_str().is_empty()
        || mapping.semantic_kind.as_str().is_empty()
        || mapping.semantic_revision.as_str().is_empty()
        || mapping.authority_grant_id.as_str().is_empty()
    {
        return Err(InteropRefusal::EmptyIdentity);
    }
    if mapping.maximum_payload_bytes == 0 || mapping.maximum_queued_items == 0 {
        return Err(InteropRefusal::InvalidBounds);
    }
    if mapping.external_delivery != mapping.declared_semantic_delivery
        || !mapping.preserves_external_lifecycle
    {
        return Err(InteropRefusal::FabricatedGuarantee);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
