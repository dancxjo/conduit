//! The portable renderer Front and host-supplied realization offer builder.

use alloc::vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    CapabilityOfferBuilder, CapabilityRealization, ExecutionProfileId, HostOperationRequirement,
    ImplementationId, ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, ResourceRequirement, SemanticCapabilityContract,
};

pub const RENDERER_KIND: &str = "presentation/renderer";
pub const INTERACTION_KIND: &str = "presentation/interaction";
pub const PRESENTATION_TEE_KIND: &str = "presentation/tee";
pub const PRESENTER_STAGE_KIND: &str = "presentation/presenter-stage";
pub const PRESENTATION_VALUE_KIND: &str = "presentation/presentation@1";
pub const MANIFESTATION_VALUE_KIND: &str = "presentation/manifestation@1";
pub const RENDERER_CONTRACT_REVISION: &str = "conduit.presentation/renderer@1";
pub const INTERACTION_CONTRACT_REVISION: &str = "conduit.presentation/interaction@1";
pub const PRESENTATION_TEE_CONTRACT_REVISION: &str = "conduit.presentation/tee@1";
pub const PRESENTER_STAGE_CONTRACT_REVISION: &str = "conduit.presentation/presenter-stage@1";
pub const MAX_RENDERER_VALUE_BYTES: u32 = crate::MAX_PRESENTATION_TOTAL_BYTES as u32;
pub const MAX_PRESENTATION_ACTIVE_INSTANCES: u16 = 8;
pub const MAX_PRESENTATION_QUEUE_ITEMS: u16 = 8;

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

pub fn interaction_inputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![
        PortDescriptor {
            port_id: port_id("presentation"),
            value_kind: kind_id(PRESENTATION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        },
        PortDescriptor {
            port_id: port_id("manifestation"),
            value_kind: kind_id(MANIFESTATION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        },
    ]
}

pub fn interaction_outputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("interaction"),
        value_kind: kind_id(crate::PRESENTATION_INTERACTION_VALUE_KIND),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
    }]
}

pub fn presentation_tee_inputs() -> alloc::vec::Vec<PortDescriptor> {
    renderer_inputs()
}

pub fn presentation_tee_outputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("presentation"),
        value_kind: kind_id(PRESENTATION_VALUE_KIND),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
    }]
}

/// A bounded linear Presenter stage transforms one portable Presentation into
/// another. A terminal `presentation/renderer` consumes the final value and
/// produces the Manifestation. The ordinary Plan Cords define ordering.
pub fn presenter_stage_inputs() -> alloc::vec::Vec<PortDescriptor> {
    renderer_inputs()
}

pub fn presenter_stage_outputs() -> alloc::vec::Vec<PortDescriptor> {
    presentation_tee_outputs()
}

pub fn presenter_stage_offer(
    capability_id: CapabilityId,
    implementation: ImplementationOffer,
    limits: CapabilityLimits,
) -> CapabilityOffer {
    build_offer(
        presenter_stage_contract(),
        capability_id,
        implementation,
        alloc::vec::Vec::new(),
        alloc::vec::Vec::new(),
        limits,
    )
}

/// Exact host-owned implementation facts beneath the one portable Front.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionRealizationOffer {
    pub capability_id: CapabilityId,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub host_operation: HostOperationRequirement,
    pub resource_requirement: ResourceRequirement,
    pub limits: CapabilityLimits,
}

pub fn renderer_offer(realization: RendererRealizationOffer) -> CapabilityOffer {
    build_offer(
        renderer_contract(),
        realization.capability_id,
        ImplementationOffer {
            execution_profile_id: realization.execution_profile_id,
            implementation_id: realization.implementation_id,
            artifact_id: realization.artifact_id,
        },
        vec![realization.host_operation],
        vec![realization.resource_requirement],
        realization.limits,
    )
}

pub fn interaction_offer(realization: InteractionRealizationOffer) -> CapabilityOffer {
    build_offer(
        interaction_contract(),
        realization.capability_id,
        ImplementationOffer {
            execution_profile_id: realization.execution_profile_id,
            implementation_id: realization.implementation_id,
            artifact_id: realization.artifact_id,
        },
        vec![realization.host_operation],
        vec![realization.resource_requirement],
        realization.limits,
    )
}

pub fn presentation_tee_offer(
    capability_id: CapabilityId,
    implementation: ImplementationOffer,
    limits: CapabilityLimits,
) -> CapabilityOffer {
    build_offer(
        presentation_tee_contract(),
        capability_id,
        implementation,
        alloc::vec::Vec::new(),
        alloc::vec::Vec::new(),
        limits,
    )
}

fn semantic_contract(
    kind: &str,
    revision: &str,
    inputs: alloc::vec::Vec<PortDescriptor>,
    outputs: alloc::vec::Vec<PortDescriptor>,
    max_queue_bytes: u32,
) -> SemanticCapabilityContract {
    SemanticCapabilityContract {
        startup_parameters: alloc::vec::Vec::new(),
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        inputs,
        outputs,
        limits: CapabilityLimits {
            max_active_instances: MAX_PRESENTATION_ACTIVE_INSTANCES,
            max_queue_items: MAX_PRESENTATION_QUEUE_ITEMS,
            max_queue_bytes,
        },
    }
}

fn renderer_contract() -> SemanticCapabilityContract {
    semantic_contract(
        RENDERER_KIND,
        RENDERER_CONTRACT_REVISION,
        renderer_inputs(),
        renderer_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_PRESENTATION_QUEUE_ITEMS),
    )
}

fn interaction_contract() -> SemanticCapabilityContract {
    semantic_contract(
        INTERACTION_KIND,
        INTERACTION_CONTRACT_REVISION,
        interaction_inputs(),
        interaction_outputs(),
        crate::MAX_PRESENTATION_INTERACTION_BYTES as u32 * u32::from(MAX_PRESENTATION_QUEUE_ITEMS),
    )
}

fn presentation_tee_contract() -> SemanticCapabilityContract {
    semantic_contract(
        PRESENTATION_TEE_KIND,
        PRESENTATION_TEE_CONTRACT_REVISION,
        presentation_tee_inputs(),
        presentation_tee_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_PRESENTATION_QUEUE_ITEMS),
    )
}

fn presenter_stage_contract() -> SemanticCapabilityContract {
    semantic_contract(
        PRESENTER_STAGE_KIND,
        PRESENTER_STAGE_CONTRACT_REVISION,
        presenter_stage_inputs(),
        presenter_stage_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_PRESENTATION_QUEUE_ITEMS),
    )
}

fn build_offer(
    contract: SemanticCapabilityContract,
    capability_id: CapabilityId,
    implementation: ImplementationOffer,
    host_operations: alloc::vec::Vec<HostOperationRequirement>,
    resource_requirements: alloc::vec::Vec<ResourceRequirement>,
    limits: CapabilityLimits,
) -> CapabilityOffer {
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id,
            execution_profile_id: implementation.execution_profile_id,
            implementation_id: implementation.implementation_id,
            artifact_id: implementation.artifact_id,
            host_operations,
            resource_requirements,
            authority_requirements: alloc::vec::Vec::new(),
        },
    )
    .narrow_capacity(limits)
    .expect("presentation realization capacity narrows portable semantics")
    .build()
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

#[cfg(feature = "form-catalog")]
pub fn interaction_kind_definition() -> conduit_form::KindDefinition {
    conduit_form::KindDefinition {
        kind_id: kind_id(INTERACTION_KIND),
        kind_contract_revision: KindContractRevision::from(INTERACTION_CONTRACT_REVISION),
        inputs: interaction_inputs(),
        outputs: interaction_outputs(),
        configuration: alloc::vec::Vec::new(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn presentation_tee_kind_definition() -> conduit_form::KindDefinition {
    conduit_form::KindDefinition {
        kind_id: kind_id(PRESENTATION_TEE_KIND),
        kind_contract_revision: KindContractRevision::from(PRESENTATION_TEE_CONTRACT_REVISION),
        inputs: presentation_tee_inputs(),
        outputs: presentation_tee_outputs(),
        configuration: alloc::vec::Vec::new(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn presenter_stage_kind_definition() -> conduit_form::KindDefinition {
    conduit_form::KindDefinition {
        kind_id: kind_id(PRESENTER_STAGE_KIND),
        kind_contract_revision: KindContractRevision::from(PRESENTER_STAGE_CONTRACT_REVISION),
        inputs: presenter_stage_inputs(),
        outputs: presenter_stage_outputs(),
        configuration: alloc::vec::Vec::new(),
    }
}
