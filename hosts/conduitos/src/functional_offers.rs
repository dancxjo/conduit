//! ConduitOS-owned realization offers for portable logic and math contracts.

use alloc::vec;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, kind_id,
};

pub const FUNCTIONAL_KERNEL_PROFILE: &str = "conduitos/functional-kernel@1";
pub const FUNCTIONAL_KERNEL_ARTIFACT: &str = "conduitos/functional-kernel@1";
pub const LOGIC_COMPARE_SCALAR_IMPLEMENTATION: &str = "conduitos/kernel-logic-compare-scalar@1";
pub const LOGIC_NOT_IMPLEMENTATION: &str = "conduitos/kernel-logic-not@1";
pub const LOGIC_SELECT_SCALAR_IMPLEMENTATION: &str = "conduitos/kernel-logic-select-scalar@1";
pub const MATH_CLAMP_IMPLEMENTATION: &str = "conduitos/kernel-math-clamp-scalar@1";

pub fn logic_compare_scalar_offer() -> CapabilityOffer {
    with_operation(
        realize(
            conduit_std_catalog::logic_compare_scalar_offer(),
            "conduitos/logic-compare-scalar@1",
            LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
        ),
        "conduit.host/logic-compare-scalar@1",
        conduit_std_catalog::LOGIC_COMPARE_KIND,
        conduit_core::SCALAR_ENCODED_LEN as u32,
        conduit_core::BOOL_ENCODED_LEN as u32,
    )
}

pub fn logic_not_offer() -> CapabilityOffer {
    with_operation(
        realize(
            conduit_std_catalog::logic_not_offer(),
            "conduitos/logic-not@1",
            LOGIC_NOT_IMPLEMENTATION,
        ),
        "conduit.host/logic-not@1",
        conduit_std_catalog::LOGIC_NOT_KIND,
        conduit_core::BOOL_ENCODED_LEN as u32,
        conduit_core::BOOL_ENCODED_LEN as u32,
    )
}

pub fn logic_select_scalar_offer() -> CapabilityOffer {
    with_operation(
        realize(
            conduit_std_catalog::logic_select_scalar_offer(),
            "conduitos/logic-select-scalar@1",
            LOGIC_SELECT_SCALAR_IMPLEMENTATION,
        ),
        "conduit.host/logic-select-scalar@1",
        conduit_std_catalog::LOGIC_SELECT_KIND,
        conduit_core::SCALAR_ENCODED_LEN as u32,
        conduit_core::SCALAR_ENCODED_LEN as u32,
    )
}

pub fn math_clamp_offer() -> CapabilityOffer {
    realize(
        conduit_std_catalog::math_clamp_offer(),
        "conduitos/math-clamp-scalar@1",
        MATH_CLAMP_IMPLEMENTATION,
    )
}

pub fn math_scale_offer() -> CapabilityOffer {
    realize(
        conduit_std_catalog::math_scale_offer(),
        "conduitos-math-scale-scalar-v1",
        "conduitos/kernel-math-scale-scalar@1",
    )
}

pub fn math_deadband_offer() -> CapabilityOffer {
    realize(
        conduit_std_catalog::math_deadband_offer(),
        "conduitos-math-deadband-scalar-v1",
        "conduitos/kernel-math-deadband-scalar@1",
    )
}

fn realize(mut offer: CapabilityOffer, capability: &str, implementation: &str) -> CapabilityOffer {
    offer.capability_id = CapabilityId::from(capability);
    offer.implementation.execution_profile_id = ExecutionProfileId::from(FUNCTIONAL_KERNEL_PROFILE);
    offer.implementation.implementation_id = ImplementationId::from(implementation);
    offer.implementation.artifact_id = ArtifactId::from(FUNCTIONAL_KERNEL_ARTIFACT);
    offer
}

fn with_operation(
    mut offer: CapabilityOffer,
    contract: &str,
    target: &str,
    input: u32,
    output: u32,
) -> CapabilityOffer {
    offer.host_operations = vec![HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: Some(kind_id(target)),
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: output,
    }];
    offer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realization_preserves_portable_contract_and_bounds() {
        for (realized, portable) in [
            (
                logic_compare_scalar_offer(),
                conduit_std_catalog::logic_compare_scalar_offer(),
            ),
            (logic_not_offer(), conduit_std_catalog::logic_not_offer()),
            (
                logic_select_scalar_offer(),
                conduit_std_catalog::logic_select_scalar_offer(),
            ),
            (math_clamp_offer(), conduit_std_catalog::math_clamp_offer()),
            (math_scale_offer(), conduit_std_catalog::math_scale_offer()),
            (
                math_deadband_offer(),
                conduit_std_catalog::math_deadband_offer(),
            ),
        ] {
            assert_eq!(realized.kind_id, portable.kind_id);
            assert_eq!(
                realized.kind_contract_revision,
                portable.kind_contract_revision
            );
            assert_eq!(realized.inputs, portable.inputs);
            assert_eq!(realized.outputs, portable.outputs);
            assert_eq!(realized.limits, portable.limits);
            assert_eq!(realized.startup_parameters, portable.startup_parameters);
            assert_eq!(
                realized.implementation.execution_profile_id.as_str(),
                FUNCTIONAL_KERNEL_PROFILE
            );
            assert_eq!(
                realized.implementation.artifact_id.as_str(),
                FUNCTIONAL_KERNEL_ARTIFACT
            );
        }
    }

    #[test]
    fn neutral_catalog_does_not_own_conduitos_realization_identity() {
        let logic = include_str!("../../../crates/conduit-std-catalog/src/logic.rs");
        let math = include_str!("../../../crates/conduit-std-catalog/src/math.rs");
        assert!(!logic.contains("conduitos/"));
        assert!(!math.contains("conduitos/"));
        assert!(!math.contains("conduitos-math"));
    }
}
