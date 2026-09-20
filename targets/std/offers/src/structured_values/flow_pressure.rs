use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId, KindId,
};

pub const FLOW_BACKPRESSURE_STD_PROFILE: &str = "std/flow-backpressure-kernel@1";
pub const FLOW_BACKPRESSURE_STD_IMPLEMENTATION: &str = "std/kernel-flow-backpressure@1";
pub const FLOW_BACKPRESSURE_STD_ARTIFACT: &str = "conduit-kernel/flow-backpressure@1";
pub const FLOW_COALESCE_LATEST_STD_PROFILE: &str = "std/flow-coalesce-latest-kernel@1";
pub const FLOW_COALESCE_LATEST_STD_IMPLEMENTATION: &str = "std/kernel-flow-coalesce-latest@1";
pub const FLOW_COALESCE_LATEST_STD_ARTIFACT: &str = "conduit-kernel/flow-coalesce-latest@1";

pub fn flow_backpressure_std_offer(
    value_kind: &KindId,
    maximum_item_bytes: u32,
) -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::flow_backpressure_contract(value_kind, maximum_item_bytes),
        "std-flow-backpressure",
        FLOW_BACKPRESSURE_STD_PROFILE,
        FLOW_BACKPRESSURE_STD_IMPLEMENTATION,
        FLOW_BACKPRESSURE_STD_ARTIFACT,
    )
}

pub fn flow_coalesce_latest_std_offer(
    value_kind: &KindId,
    maximum_item_bytes: u32,
) -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::flow_coalesce_latest_contract(value_kind, maximum_item_bytes),
        "std-flow-coalesce-latest",
        FLOW_COALESCE_LATEST_STD_PROFILE,
        FLOW_COALESCE_LATEST_STD_IMPLEMENTATION,
        FLOW_COALESCE_LATEST_STD_ARTIFACT,
    )
}

fn offer(
    contract: conduit_semantic_catalog::FlowPressureContract,
    capability_prefix: &str,
    execution_profile: &str,
    implementation: &str,
    artifact: &str,
) -> CapabilityOffer {
    let value_kind = contract
        .outputs
        .first()
        .or_else(|| contract.inputs.first())
        .expect("flow pressure contract has one runtime port")
        .value_kind
        .as_str()
        .to_string();
    BackOfferBuilder::new(
        contract.into(),
        Back {
            capability_id: CapabilityId::from(format!("{capability_prefix}-{value_kind}")),
            execution_profile_id: ExecutionProfileId::from(execution_profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, DeliveryPressurePolicy, SCALAR_ENCODED_LEN, SCALAR_INFO_ID};

    #[test]
    fn offers_preserve_exact_checked_pressure_contracts() {
        let value_kind = kind_id(SCALAR_INFO_ID);
        let backpressure = flow_backpressure_std_offer(&value_kind, SCALAR_ENCODED_LEN as u32);
        let coalesce = flow_coalesce_latest_std_offer(&value_kind, SCALAR_ENCODED_LEN as u32);
        assert_eq!(
            conduit_semantic_catalog::reviewed_flow_pressure_policy(&backpressure.kind_id),
            Some(DeliveryPressurePolicy::PreserveOrder)
        );
        assert_eq!(
            conduit_semantic_catalog::reviewed_flow_pressure_policy(&coalesce.kind_id),
            Some(DeliveryPressurePolicy::CoalesceLatest)
        );
        assert_eq!(backpressure.inputs[0].value_kind, value_kind);
        assert_eq!(coalesce.outputs[0].value_kind, value_kind);
    }
}
