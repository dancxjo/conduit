//! Explicit finite State offer; callers must install its implementation before advertising it.
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, ImplementationId,
    ImplementationOffer, StructuredInfoRefusal, StructuredInfoType,
};

pub const STATE_VALUE_STD_PROFILE: &str = "std/state-value-kernel-64@1";
pub const STATE_VALUE_STD_IMPLEMENTATION: &str = "std/kernel-state-value-64@1";
pub const STATE_VALUE_STD_ARTIFACT: &str = "conduit-kernel/state-delay@1";
pub const STATE_VALUE_STD_MAXIMUM_BYTES: u32 = 64;

/// Construct the exact offer for the kernel's finite canonical-value envelope.
/// This does not register a capability or authorize an effect. The installation
/// must independently validate the sealed State contract before Play starts.
pub fn state_value_std_offer(
    type_name: &str,
    value_type: &StructuredInfoType,
) -> Result<CapabilityOffer, StructuredInfoRefusal> {
    let mut contract =
        conduit_semantic_catalog::state_value::state_value_contract(type_name, value_type)?;
    contract.limits.max_queue_bytes = STATE_VALUE_STD_MAXIMUM_BYTES;
    let value_kind = contract.outputs[0].value_kind.as_str();
    Ok(CapabilityOffer {
        capability_id: CapabilityId::from(format!("std-state-value-{value_kind}")),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        startup_parameters: contract.startup_parameters,
        shorthand: Some((
            conduit_core::port_id("next"),
            conduit_core::port_id("current"),
        )),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(STATE_VALUE_STD_PROFILE),
            implementation_id: ImplementationId::from(STATE_VALUE_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(STATE_VALUE_STD_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offer_preserves_typed_meaning_and_names_its_smaller_realization_bound() {
        let ty =
            StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID)).unwrap();
        let contract =
            conduit_semantic_catalog::state_value::state_value_contract("Cell", &ty).unwrap();
        let offer = state_value_std_offer("Cell", &ty).unwrap();
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.startup_parameters, contract.startup_parameters);
        assert_eq!(
            offer.kind_contract_revision,
            contract.kind_contract_revision
        );
        assert_eq!(offer.limits.max_queue_bytes, STATE_VALUE_STD_MAXIMUM_BYTES);
        assert!(offer.limits.max_queue_bytes < contract.limits.max_queue_bytes);
        assert!(offer.host_operations.is_empty());
        assert!(offer.authority_requirements.is_empty());
    }
}
