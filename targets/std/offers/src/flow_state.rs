//! Hosted std realizations of portable flow/state contracts.

use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, Kind,
};

pub const STATE_LATEST_SCALAR_EXECUTION_PROFILE: &str = "conduit.std/state-latest-scalar-kernel@2";
pub const STATE_LATEST_SCALAR_IMPLEMENTATION: &str = "std/kernel-state-latest-scalar@2";
pub const STATE_LATEST_SCALAR_ARTIFACT: &str = "conduit-std-host/state-latest-scalar@2";
pub const FLOW_TEE_SCALAR_EXECUTION_PROFILE: &str = "conduit.std/flow-tee-scalar-kernel@2";
pub const FLOW_TEE_SCALAR_IMPLEMENTATION: &str = "std/kernel-flow-tee-scalar@2";
pub const FLOW_TEE_SCALAR_ARTIFACT: &str = "conduit-std-host/flow-tee-scalar@2";
pub const FLOW_GATE_SCALAR_EXECUTION_PROFILE: &str = "conduit.std/flow-gate-scalar-kernel@1";
pub const FLOW_GATE_SCALAR_IMPLEMENTATION: &str = "std/kernel-flow-gate-scalar@1";
pub const FLOW_GATE_SCALAR_ARTIFACT: &str = "conduit-std-host/flow-gate-scalar@1";
pub const FLOW_GATE_BOOL_HOST_OPERATION_CONTRACT: &str = "conduit.host/decode-bool@1";
pub const FLOW_GATE_BOOL_HOST_OPERATION_TARGET: &str = "value/decode-bool";
pub const STATE_SELECT_SCALAR_EXECUTION_PROFILE: &str = "conduit.std/state-select-scalar-kernel@1";
pub const STATE_SELECT_SCALAR_IMPLEMENTATION: &str = "std/kernel-state-select-scalar@1";
pub const STATE_SELECT_SCALAR_ARTIFACT: &str = "conduit-std-host/state-select-scalar@1";

pub fn state_latest_scalar_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::state_latest_scalar_semantic_contract(),
        "state-latest-scalar-v2",
        STATE_LATEST_SCALAR_EXECUTION_PROFILE,
        STATE_LATEST_SCALAR_IMPLEMENTATION,
        STATE_LATEST_SCALAR_ARTIFACT,
        Vec::new(),
    )
}

pub fn flow_tee_scalar_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::flow_tee_scalar_semantic_contract(),
        "flow-tee-scalar-v2",
        FLOW_TEE_SCALAR_EXECUTION_PROFILE,
        FLOW_TEE_SCALAR_IMPLEMENTATION,
        FLOW_TEE_SCALAR_ARTIFACT,
        Vec::new(),
    )
}

pub fn flow_gate_scalar_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::flow_gate_scalar_semantic_contract(),
        "flow-gate-scalar-v1",
        FLOW_GATE_SCALAR_EXECUTION_PROFILE,
        FLOW_GATE_SCALAR_IMPLEMENTATION,
        FLOW_GATE_SCALAR_ARTIFACT,
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(FLOW_GATE_BOOL_HOST_OPERATION_CONTRACT),
            target_kind: Some(kind_id(FLOW_GATE_BOOL_HOST_OPERATION_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: 1,
        }],
    )
}

pub fn state_select_scalar_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::state_select_scalar_semantic_contract(),
        "state-select-scalar-v1",
        STATE_SELECT_SCALAR_EXECUTION_PROFILE,
        STATE_SELECT_SCALAR_IMPLEMENTATION,
        STATE_SELECT_SCALAR_ARTIFACT,
        Vec::new(),
    )
}

fn offer(
    contract: Kind,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    host_operations: Vec<HostOperationRequirement>,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_operations,
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_preserve_exact_portable_contracts() {
        for (offer, contract) in [
            (
                state_latest_scalar_offer(),
                conduit_semantic_catalog::state_latest_scalar_contract(),
            ),
            (
                flow_tee_scalar_offer(),
                conduit_semantic_catalog::flow_tee_scalar_contract(),
            ),
            (
                flow_gate_scalar_offer(),
                conduit_semantic_catalog::flow_gate_scalar_contract(),
            ),
            (
                state_select_scalar_offer(),
                conduit_semantic_catalog::state_select_scalar_contract(),
            ),
        ] {
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
        }
        assert_eq!(flow_gate_scalar_offer().host_operations.len(), 1);
    }
}
