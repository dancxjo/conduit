//! Portable deliberate level input and exact three-way Signal merge contracts.
//!
//! Platform adapters may fulfill an admitted level request from a terminal or
//! browser event. They do not own the resulting Signal or merge ordering.

use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, HostCallContractId, HostCallRequirement, ImplementationId,
    Kind, KindId, KindIdentity, PortDescriptor, PortDirection, PortTemporal, ResourceRequirement,
    INPUT_RESOURCE_CLASS,
};

use crate::{signal_value_kind, SIGNAL_ENCODED_LEN, SIGNAL_PORT};

pub const LEVEL_INPUT_KIND: &str = "interaction/level";
pub const MERGE_THREE_SIGNAL_KIND: &str = "flow/merge-three-signal";
pub const AWAIT_LEVEL_HOST_CALL_CONTRACT: &str = "conduit.host/await-level@1";
pub const LEVEL_INPUT_CONTRACT_REVISION: &str = "conduit.signal/interaction-level@1";
pub const MERGE_THREE_SIGNAL_CONTRACT_REVISION: &str = "conduit.signal/flow-merge-three-signal@1";
pub const LEVEL_INPUT_EXECUTION_PROFILE: &str = "conduit.signal/level-input-hosted@1";
pub const MERGE_THREE_SIGNAL_EXECUTION_PROFILE: &str = "conduit.signal/merge-three-signal-kernel@1";
pub const TERMINAL_INPUT_PORT: &str = "terminal";
pub const BROWSER_A_INPUT_PORT: &str = "browser-a";
pub const BROWSER_B_INPUT_PORT: &str = "browser-b";

pub fn level_input_kind() -> KindId {
    kind_id(LEVEL_INPUT_KIND)
}

pub fn merge_three_signal_kind() -> KindId {
    kind_id(MERGE_THREE_SIGNAL_KIND)
}

pub fn level_input_contract_revision() -> KindIdentity {
    KindIdentity::from(LEVEL_INPUT_CONTRACT_REVISION)
}

pub fn merge_three_signal_contract_revision() -> KindIdentity {
    KindIdentity::from(MERGE_THREE_SIGNAL_CONTRACT_REVISION)
}

pub fn level_input_outputs() -> Vec<PortDescriptor> {
    vec![signal_port(SIGNAL_PORT, PortDirection::Output)]
}

pub fn merge_three_signal_inputs() -> Vec<PortDescriptor> {
    vec![
        signal_port(TERMINAL_INPUT_PORT, PortDirection::Input),
        signal_port(BROWSER_A_INPUT_PORT, PortDirection::Input),
        signal_port(BROWSER_B_INPUT_PORT, PortDirection::Input),
    ]
}

pub fn merge_three_signal_outputs() -> Vec<PortDescriptor> {
    vec![signal_port(SIGNAL_PORT, PortDirection::Output)]
}

pub fn await_level_host_call_requirement() -> HostCallRequirement {
    HostCallRequirement {
        contract_id: HostCallContractId::from(AWAIT_LEVEL_HOST_CALL_CONTRACT),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: 1,
        maximum_output_bytes: 1,
    }
}

pub fn level_input_resource_requirements() -> Vec<ResourceRequirement> {
    vec![conduit_core::resource_requirement(INPUT_RESOURCE_CLASS, 1)]
}

pub fn level_input_capability(
    capability_id: &str,
    implementation_id: &str,
    maximum_instances: u16,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        level_input_semantic_contract(maximum_instances),
        Back {
            capability_id: CapabilityId::from(capability_id),
            execution_profile_id: ExecutionProfileId::from(LEVEL_INPUT_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(implementation_id),
            artifact_id: ArtifactId::from("conduit-signal/level-input-artifact-v1"),
            host_calls: vec![await_level_host_call_requirement()],
            resource_requirements: level_input_resource_requirements(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

pub fn level_input_semantic_contract(maximum_instances: u16) -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: level_input_kind(),
        kind_contract_revision: level_input_contract_revision(),
        inputs: Vec::new(),
        outputs: level_input_outputs(),
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: maximum_instances,
            max_queue_items: 1,
            max_queue_bytes: SIGNAL_ENCODED_LEN,
        },
    }
}

pub fn merge_three_signal_capability(
    capability_id: &str,
    implementation_id: &str,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        merge_three_signal_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(capability_id),
            execution_profile_id: ExecutionProfileId::from(MERGE_THREE_SIGNAL_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(implementation_id),
            artifact_id: ArtifactId::from("conduit-signal/merge-three-signal-artifact-v1"),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

pub fn merge_three_signal_semantic_contract() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: merge_three_signal_kind(),
        kind_contract_revision: merge_three_signal_contract_revision(),
        inputs: merge_three_signal_inputs(),
        outputs: merge_three_signal_outputs(),
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: 3 * SIGNAL_ENCODED_LEN,
        },
    }
}

pub(crate) fn extend_control_profile_catalog(catalog: &mut conduit_form::ProfileCatalog) {
    use conduit_form::KindProjection;

    for capability in [
        level_input_capability("catalog/level-input", "catalog/level-input@1", 1),
        merge_three_signal_capability("catalog/merge-three", "catalog/merge-three@1"),
    ] {
        catalog
            .insert(KindProjection {
                kind_id: capability.kind_id,
                kind_contract_revision: capability.kind_contract_revision,
                inputs: capability.inputs,
                outputs: capability.outputs,
                configuration: Default::default(),
            })
            .expect("Signal control profile kinds are unique");
    }
}

fn signal_port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: signal_value_kind(),
        direction,
        temporal: PortTemporal::Value,
    }
}
