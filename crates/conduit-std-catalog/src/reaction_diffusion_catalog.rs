//! Canonical Form contract and truthful finite std offer for field evolution.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    REACTION_DIFFUSION_MAXIMUM_STATE_BYTES, REACTION_DIFFUSION_REQUEST_INFO_ID,
    REACTION_DIFFUSION_STATE_INFO_ID,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::HOSTED_REACTION_DIFFUSION_LIMITS;

pub const REACTION_DIFFUSION_EVOLVE_KIND: &str = "field/evolve";
pub const REACTION_DIFFUSION_KIND_REVISION: &str = "conduit.std/field-evolve@1";
pub const REACTION_DIFFUSION_HOSTED_PROFILE: &str = "std/field-gray-scott-hosted@1";
pub const REACTION_DIFFUSION_HOSTED_ARTIFACT: &str = "conduit-std-host/field-gray-scott@1";
pub const REACTION_DIFFUSION_HOST_OPERATION: &str = "conduit.host/field-evolve@1";

pub fn install_reaction_diffusion_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    let inputs = evolve_inputs();
    let outputs = evolve_outputs();
    startup
        .insert(KindSignature {
            kind: REACTION_DIFFUSION_EVOLVE_KIND.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(REACTION_DIFFUSION_EVOLVE_KIND),
            kind_contract_revision: KindContractRevision::from(REACTION_DIFFUSION_KIND_REVISION),
            inputs,
            outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

pub fn reaction_diffusion_std_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("std/field-evolve@1"),
        kind_id: kind_id(REACTION_DIFFUSION_EVOLVE_KIND),
        kind_contract_revision: KindContractRevision::from(REACTION_DIFFUSION_KIND_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(REACTION_DIFFUSION_HOSTED_PROFILE),
            implementation_id: ImplementationId::from("std/field-gray-scott@1"),
            artifact_id: ArtifactId::from(REACTION_DIFFUSION_HOSTED_ARTIFACT),
        },
        inputs: evolve_inputs(),
        outputs: evolve_outputs(),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(REACTION_DIFFUSION_HOST_OPERATION),
            target_kind: Some(kind_id(REACTION_DIFFUSION_EVOLVE_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: HOSTED_REACTION_DIFFUSION_LIMITS.maximum_input_bytes,
            maximum_output_bytes: HOSTED_REACTION_DIFFUSION_LIMITS.maximum_output_bytes,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: HOSTED_REACTION_DIFFUSION_LIMITS.maximum_active_instances,
            max_queue_items: HOSTED_REACTION_DIFFUSION_LIMITS.maximum_queued_requests,
            max_queue_bytes: REACTION_DIFFUSION_MAXIMUM_STATE_BYTES,
        },
    }
}

fn evolve_inputs() -> Vec<PortDescriptor> {
    vec![
        value_port(
            "state",
            REACTION_DIFFUSION_STATE_INFO_ID,
            PortDirection::Input,
        ),
        value_port(
            "request",
            REACTION_DIFFUSION_REQUEST_INFO_ID,
            PortDirection::Input,
        ),
    ]
}

fn evolve_outputs() -> Vec<PortDescriptor> {
    vec![value_port(
        "next-state",
        REACTION_DIFFUSION_STATE_INFO_ID,
        PortDirection::Output,
    )]
}

fn value_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}
