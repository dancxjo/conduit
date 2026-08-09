//! The portable renderer Face and host-supplied realization offer builder.

use alloc::vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal, ResourceRequirement,
};

pub const RENDERER_KIND: &str = "presentation/renderer";
pub const PRESENTATION_VALUE_KIND: &str = "presentation/presentation@1";
pub const MANIFESTATION_VALUE_KIND: &str = "presentation/manifestation@1";
pub const RENDERER_CONTRACT_REVISION: &str = "conduit.presentation/renderer@1";
pub const MAX_RENDERER_VALUE_BYTES: u32 = crate::MAX_PRESENTATION_TOTAL_BYTES as u32;

pub fn renderer_inputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("presentation"),
        value_kind: kind_id(PRESENTATION_VALUE_KIND),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
    }]
}

pub fn renderer_outputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("manifestation"),
        value_kind: kind_id(MANIFESTATION_VALUE_KIND),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
    }]
}

/// Exact host-owned implementation facts beneath the one portable Face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererRealizationOffer {
    pub capability_id: CapabilityId,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub host_operation: HostOperationRequirement,
    pub resource_requirement: ResourceRequirement,
    pub limits: CapabilityLimits,
}

pub fn renderer_offer(realization: RendererRealizationOffer) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: alloc::vec::Vec::new(),
        shorthand: None,
        capability_id: realization.capability_id,
        kind_id: kind_id(RENDERER_KIND),
        kind_contract_revision: KindContractRevision::from(RENDERER_CONTRACT_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: realization.execution_profile_id,
            implementation_id: realization.implementation_id,
            artifact_id: realization.artifact_id,
        },
        inputs: renderer_inputs(),
        outputs: renderer_outputs(),
        host_operations: vec![realization.host_operation],
        resource_requirements: vec![realization.resource_requirement],
        authority_requirements: alloc::vec::Vec::new(),
        limits: realization.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn renderer_kind_definition() -> conduit_form::KindDefinition {
    conduit_form::KindDefinition {
        kind_id: kind_id(RENDERER_KIND),
        kind_contract_revision: KindContractRevision::from(RENDERER_CONTRACT_REVISION),
        inputs: renderer_inputs(),
        outputs: renderer_outputs(),
        configuration: alloc::vec::Vec::new(),
    }
}
