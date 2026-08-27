//! Exact structured-selector realization offers owned by the hosted std Host.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, PortTemporal,
    StructuredSelector, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const STRUCTURED_SELECTOR_STD_PROFILE: &str = "std/structured-selector-kernel-hosted@1";
pub const STRUCTURED_SELECTOR_STD_IMPLEMENTATION: &str = "std/kernel-structured-selector@1";
pub const STRUCTURED_SELECTOR_STD_ARTIFACT: &str = "conduit-core/structured-selector@1";
pub const STRUCTURED_SELECTOR_HOST_OPERATION: &str = "conduit.host/structured-selector@1";

pub fn structured_selector_std_offer(
    selector: &StructuredSelector,
    temporal: PortTemporal,
) -> CapabilityOffer {
    let contract = conduit_std_catalog::structured_selector_contract(selector, temporal);
    let digest = contract
        .kind_id
        .as_str()
        .strip_prefix("structured-info/selector-")
        .and_then(|value| value.strip_suffix(&format!("-{}@1", temporal.as_str())))
        .expect("structured selector kind identity is canonical");
    let target_kind = contract.kind_id.clone();
    CapabilityOffer {
        startup_parameters: contract.startup_parameters,
        shorthand: contract.shorthand,
        capability_id: CapabilityId::from(format!(
            "std-structured-selector-{digest}-{}",
            temporal.as_str()
        )),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(STRUCTURED_SELECTOR_STD_PROFILE),
            implementation_id: ImplementationId::from(STRUCTURED_SELECTOR_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(STRUCTURED_SELECTOR_STD_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(STRUCTURED_SELECTOR_HOST_OPERATION),
            target_kind: Some(target_kind),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_dynamic_portable_contract() {
        let selector = conduit_core::StructuredSelector::field(
            conduit_std_catalog::copy_result_type(),
            "outcome",
        )
        .unwrap();
        let contract =
            conduit_std_catalog::structured_selector_contract(&selector, PortTemporal::Value);
        let offer = structured_selector_std_offer(&selector, PortTemporal::Value);
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            contract.kind_contract_revision
        );
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
    }
}
