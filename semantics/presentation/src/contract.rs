//! The portable renderer Front and host-supplied realization offer builder.

use alloc::vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, HostCallRequirement, ImplementationId,
    ImplementationOffer, Kind, KindIdentity, KindSemanticLaw, PortDescriptor, PortDirection,
    PortTemporal, ResourcePortContract, ResourcePortLifecycle, ResourcePortMobility,
    ResourcePortOwnership, ResourceRequirement,
};

pub const RENDERER_KIND: &str = "presentation/renderer";
pub const FACE_INTERACTION_KIND: &str = "face/interaction";
pub const PRESENTATION_TEE_KIND: &str = "presentation/tee";
pub const PRESENTER_STAGE_KIND: &str = "presentation/presenter-stage";
pub const SHOW_RESOURCE_SOURCE_KIND: &str = "presentation/show-resource-source";
pub const RESOURCE_RENDERER_KIND: &str = "presentation/resource-renderer";
pub const PRESENTATION_VALUE_KIND: &str = "presentation/presentation@1";
pub const SHOW_VALUE_KIND: &str = "presentation/show@1";
pub const SHOW_RESOURCE_VALUE_KIND: &str = "presentation/show-resource@1";
pub const SHOW_RESOURCE_CLASS: &str = "presentation.resource/show@1";
/// Compatibility name for Rust callers while the internal Manifestation type
/// is migrated to the canonical Show vocabulary. It names the Show semantic
/// value and does not preserve the superseded authored identity.
pub const MANIFESTATION_VALUE_KIND: &str = SHOW_VALUE_KIND;
pub const RENDERER_CONTRACT_REVISION: &str = "conduit.presentation/renderer@1";
pub const FACE_INTERACTION_CONTRACT_REVISION: &str = "conduit.face/interaction@2";
pub const PRESENTATION_TEE_CONTRACT_REVISION: &str = "conduit.presentation/tee@1";
pub const PRESENTER_STAGE_CONTRACT_REVISION: &str = "conduit.presentation/presenter-stage@1";
pub const SHOW_RESOURCE_SOURCE_CONTRACT_REVISION: &str =
    "conduit.presentation/show-resource-source@1";
pub const RESOURCE_RENDERER_CONTRACT_REVISION: &str = "conduit.presentation/resource-renderer@1";
pub const MAX_RENDERER_VALUE_BYTES: u32 = crate::MAX_PRESENTATION_TOTAL_BYTES as u32;
pub const MAX_PRESENTATION_ACTIVE_INSTANCES: u16 = 8;
pub const MAX_PRESENTATION_QUEUE_ITEMS: u16 = 8;

pub fn renderer_inputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("presentation"),
        value_kind: kind_id(PRESENTATION_VALUE_KIND),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }]
}

pub fn renderer_outputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("show"),
        value_kind: kind_id(SHOW_VALUE_KIND),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }]
}

fn show_resource_port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("show-resource"),
        value_kind: kind_id(SHOW_RESOURCE_VALUE_KIND),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

fn show_resource_contract() -> ResourcePortContract {
    ResourcePortContract {
        port_id: port_id("show-resource"),
        class_id: conduit_core::ResourceClassId::from(SHOW_RESOURCE_CLASS),
        ownership: ResourcePortOwnership::Move,
        lifecycle: ResourcePortLifecycle::Play,
        mobility: ResourcePortMobility::HostLocal,
    }
}

pub fn resource_renderer_inputs() -> alloc::vec::Vec<PortDescriptor> {
    let mut inputs = renderer_inputs();
    inputs.push(show_resource_port(PortDirection::Input));
    inputs
}

pub fn show_resource_source_outputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![show_resource_port(PortDirection::Output)]
}

pub fn interaction_inputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![
        PortDescriptor {
            port_id: port_id("presentation"),
            value_kind: kind_id(PRESENTATION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        },
        PortDescriptor {
            port_id: port_id("show"),
            value_kind: kind_id(SHOW_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        },
    ]
}

pub fn interaction_outputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("interaction"),
        value_kind: kind_id(crate::FACE_INTERACTION_VALUE_KIND),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    }]
}

pub fn presentation_tee_inputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("source"),
        value_kind: kind_id(PRESENTATION_VALUE_KIND),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }]
}

pub fn presentation_tee_outputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("presentation"),
        value_kind: kind_id(PRESENTATION_VALUE_KIND),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }]
}

/// A bounded linear Presenter stage transforms one portable Presentation into
/// another. A terminal `presentation/renderer` consumes the final value and
/// produces the Manifestation. The ordinary plan Cords define ordering.
pub fn presenter_stage_inputs() -> alloc::vec::Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("source"),
        value_kind: kind_id(PRESENTATION_VALUE_KIND),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }]
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

/// Exact host-owned implementation facts beneath the one portable front.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererRealizationOffer {
    pub capability_id: CapabilityId,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub host_call: HostCallRequirement,
    pub resource_requirement: ResourceRequirement,
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceInteractionRealizationOffer {
    pub capability_id: CapabilityId,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub host_call: HostCallRequirement,
    pub resource_requirement: ResourceRequirement,
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowResourceSourceOffer {
    pub capability_id: CapabilityId,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub resource_requirement: ResourceRequirement,
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRendererRealizationOffer {
    pub capability_id: CapabilityId,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub host_call: HostCallRequirement,
    pub authority_requirement: conduit_core::AuthorityRequirement,
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
        vec![realization.host_call],
        vec![realization.resource_requirement],
        realization.limits,
    )
}

pub fn show_resource_source_offer(realization: ShowResourceSourceOffer) -> CapabilityOffer {
    build_offer(
        show_resource_source_contract(),
        realization.capability_id,
        ImplementationOffer {
            execution_profile_id: realization.execution_profile_id,
            implementation_id: realization.implementation_id,
            artifact_id: realization.artifact_id,
        },
        vec![],
        vec![realization.resource_requirement],
        realization.limits,
    )
}

pub fn resource_renderer_offer(realization: ResourceRendererRealizationOffer) -> CapabilityOffer {
    let mut offer = build_offer(
        resource_renderer_contract(),
        realization.capability_id,
        ImplementationOffer {
            execution_profile_id: realization.execution_profile_id,
            implementation_id: realization.implementation_id,
            artifact_id: realization.artifact_id,
        },
        vec![realization.host_call],
        vec![],
        realization.limits,
    );
    offer.authority_requirements = vec![realization.authority_requirement];
    offer
}

pub fn face_interaction_offer(realization: FaceInteractionRealizationOffer) -> CapabilityOffer {
    build_offer(
        interaction_contract(),
        realization.capability_id,
        ImplementationOffer {
            execution_profile_id: realization.execution_profile_id,
            implementation_id: realization.implementation_id,
            artifact_id: realization.artifact_id,
        },
        vec![realization.host_call],
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
) -> Kind {
    Kind {
        startup_parameters: alloc::vec::Vec::new(),
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(revision),
        inputs,
        outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: MAX_PRESENTATION_ACTIVE_INSTANCES,
            max_queue_items: MAX_PRESENTATION_QUEUE_ITEMS,
            max_queue_bytes,
        },
    }
}

fn renderer_contract() -> Kind {
    semantic_contract(
        RENDERER_KIND,
        RENDERER_CONTRACT_REVISION,
        renderer_inputs(),
        renderer_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_PRESENTATION_QUEUE_ITEMS),
    )
}

fn show_resource_source_contract() -> Kind {
    let mut contract = semantic_contract(
        SHOW_RESOURCE_SOURCE_KIND,
        SHOW_RESOURCE_SOURCE_CONTRACT_REVISION,
        alloc::vec::Vec::new(),
        show_resource_source_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_PRESENTATION_QUEUE_ITEMS),
    );
    contract
        .semantic_laws
        .push(KindSemanticLaw::ResourcePorts(vec![
            show_resource_contract(),
        ]));
    contract
}

fn resource_renderer_contract() -> Kind {
    let mut contract = semantic_contract(
        RESOURCE_RENDERER_KIND,
        RESOURCE_RENDERER_CONTRACT_REVISION,
        resource_renderer_inputs(),
        renderer_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_PRESENTATION_QUEUE_ITEMS),
    );
    contract
        .semantic_laws
        .push(KindSemanticLaw::ResourcePorts(vec![
            show_resource_contract(),
        ]));
    contract
}

fn interaction_contract() -> Kind {
    semantic_contract(
        FACE_INTERACTION_KIND,
        FACE_INTERACTION_CONTRACT_REVISION,
        interaction_inputs(),
        interaction_outputs(),
        crate::MAX_FACE_INTERACTION_BYTES as u32 * u32::from(MAX_PRESENTATION_QUEUE_ITEMS),
    )
}

fn presentation_tee_contract() -> Kind {
    semantic_contract(
        PRESENTATION_TEE_KIND,
        PRESENTATION_TEE_CONTRACT_REVISION,
        presentation_tee_inputs(),
        presentation_tee_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_PRESENTATION_QUEUE_ITEMS),
    )
}

fn presenter_stage_contract() -> Kind {
    semantic_contract(
        PRESENTER_STAGE_KIND,
        PRESENTER_STAGE_CONTRACT_REVISION,
        presenter_stage_inputs(),
        presenter_stage_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_PRESENTATION_QUEUE_ITEMS),
    )
}

fn build_offer(
    contract: Kind,
    capability_id: CapabilityId,
    implementation: ImplementationOffer,
    host_calls: alloc::vec::Vec<HostCallRequirement>,
    resource_requirements: alloc::vec::Vec<ResourceRequirement>,
    limits: CapabilityLimits,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id,
            execution_profile_id: implementation.execution_profile_id,
            implementation_id: implementation.implementation_id,
            artifact_id: implementation.artifact_id,
            host_calls,
            resource_requirements,
            authority_requirements: alloc::vec::Vec::new(),
        },
    )
    .narrow_capacity(limits)
    .expect("presentation realization capacity narrows portable semantics")
    .build()
}

#[cfg(feature = "plot-catalog")]
pub fn renderer_kind_projection() -> conduit_plot::KindProjection {
    conduit_plot::KindProjection {
        kind_id: kind_id(RENDERER_KIND),
        kind_contract_revision: KindIdentity::from(RENDERER_CONTRACT_REVISION),
        inputs: renderer_inputs(),
        outputs: renderer_outputs(),
        configuration: Default::default(),
    }
}

#[cfg(feature = "plot-catalog")]
pub fn show_resource_source_kind() -> Kind {
    show_resource_source_contract()
}

#[cfg(feature = "plot-catalog")]
pub fn resource_renderer_kind() -> Kind {
    resource_renderer_contract()
}

#[cfg(feature = "plot-catalog")]
pub fn face_interaction_kind_projection() -> conduit_plot::KindProjection {
    conduit_plot::KindProjection {
        kind_id: kind_id(FACE_INTERACTION_KIND),
        kind_contract_revision: KindIdentity::from(FACE_INTERACTION_CONTRACT_REVISION),
        inputs: interaction_inputs(),
        outputs: interaction_outputs(),
        configuration: Default::default(),
    }
}

#[cfg(feature = "plot-catalog")]
pub fn presentation_tee_kind_projection() -> conduit_plot::KindProjection {
    conduit_plot::KindProjection {
        kind_id: kind_id(PRESENTATION_TEE_KIND),
        kind_contract_revision: KindIdentity::from(PRESENTATION_TEE_CONTRACT_REVISION),
        inputs: presentation_tee_inputs(),
        outputs: presentation_tee_outputs(),
        configuration: Default::default(),
    }
}

#[cfg(feature = "plot-catalog")]
pub fn presenter_stage_kind_projection() -> conduit_plot::KindProjection {
    conduit_plot::KindProjection {
        kind_id: kind_id(PRESENTER_STAGE_KIND),
        kind_contract_revision: KindIdentity::from(PRESENTER_STAGE_CONTRACT_REVISION),
        inputs: presenter_stage_inputs(),
        outputs: presenter_stage_outputs(),
        configuration: Default::default(),
    }
}

/// Install the portable Kind fronts used by ordinary Plots serving as Masks.
///
/// This is checking truth only. A Host still has to offer and the Plan still
/// has to select each exact renderer, tee, and Face-interaction Back.
#[cfg(feature = "plot-catalog")]
pub fn install_mask_mechanism_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    for projection in [
        renderer_kind_projection(),
        face_interaction_kind_projection(),
        presentation_tee_kind_projection(),
    ] {
        startup.insert(conduit_plot::KindSignature {
            kind: projection.kind_id.as_str().into(),
            startup_parameters: alloc::vec::Vec::new(),
        })?;
        profiles
            .insert(projection)
            .map_err(|error| alloc::format!("install Mask mechanism profile: {error:?}"))?;
    }
    for kind in [show_resource_source_kind(), resource_renderer_kind()] {
        startup.insert(conduit_plot::KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: alloc::vec::Vec::new(),
        })?;
        profiles
            .insert_kind(kind)
            .map_err(|error| alloc::format!("install Mask mechanism profile: {error:?}"))?;
    }
    Ok(())
}
