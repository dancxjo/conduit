//! Hosted std realizations of portable flow/state contracts.

use conduit_core::{kind_id, CapabilityOffer, HostOperationContractId, HostOperationRequirement};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};

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
        conduit_semantic_catalog::state_latest_scalar_contract(),
        conduit_semantic_catalog::STATE_LATEST_SCALAR_CONTRACT_REVISION,
        "state-latest-scalar-v2",
        STATE_LATEST_SCALAR_EXECUTION_PROFILE,
        STATE_LATEST_SCALAR_IMPLEMENTATION,
        STATE_LATEST_SCALAR_ARTIFACT,
        Vec::new(),
    )
}

pub fn flow_tee_scalar_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::flow_tee_scalar_contract(),
        conduit_semantic_catalog::FLOW_TEE_SCALAR_CONTRACT_REVISION,
        "flow-tee-scalar-v2",
        FLOW_TEE_SCALAR_EXECUTION_PROFILE,
        FLOW_TEE_SCALAR_IMPLEMENTATION,
        FLOW_TEE_SCALAR_ARTIFACT,
        Vec::new(),
    )
}

pub fn flow_gate_scalar_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::flow_gate_scalar_contract(),
        conduit_semantic_catalog::FLOW_GATE_SCALAR_CONTRACT_REVISION,
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
        conduit_semantic_catalog::state_select_scalar_contract(),
        conduit_semantic_catalog::STATE_SELECT_SCALAR_CONTRACT_REVISION,
        "state-select-scalar-v1",
        STATE_SELECT_SCALAR_EXECUTION_PROFILE,
        STATE_SELECT_SCALAR_IMPLEMENTATION,
        STATE_SELECT_SCALAR_ARTIFACT,
        Vec::new(),
    )
}

fn offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    revision: &str,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    host_operations: Vec<HostOperationRequirement>,
) -> CapabilityOffer {
    realization_offer(
        contract,
        revision,
        RealizationOfferIdentity {
            capability,
            execution_profile: profile,
            implementation,
            artifact,
        },
        host_operations,
        Vec::new(),
        Vec::new(),
    )
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
