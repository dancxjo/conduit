//! Std Host realization of reusable correlated delivery-status projection.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId,
};

pub const RECORD_DELIVERY_STATUS_STD_IMPLEMENTATION: &str = "std/record-delivery-status@1";
pub const RECORD_DELIVERY_STATUS_HOST_OPERATION: &str = "conduit.host/record-delivery-status@1";

pub fn record_delivery_status_std_offer() -> CapabilityOffer {
    let contract = conduit_net::record_delivery_status_semantic_contract();
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(RECORD_DELIVERY_STATUS_STD_IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from("std/record-delivery-status@1"),
            implementation_id: ImplementationId::from(RECORD_DELIVERY_STATUS_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-net/record-delivery-status@1"),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(RECORD_DELIVERY_STATUS_HOST_OPERATION),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_net::MAXIMUM_RECORD_DELIVERY_CANONICAL_BYTES as u32,
                maximum_output_bytes: conduit_net::MAXIMUM_RECORD_DELIVERY_CANONICAL_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
