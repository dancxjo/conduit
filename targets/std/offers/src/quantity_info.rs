//! Exact hosted wrapping into the existing structured Quantity presentation type.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder, CapabilityRealization,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
};

pub const QUANTITY_INFO_IMPLEMENTATION: &str = "std/kernel-wrap-quantity@1";
pub const QUANTITY_INFO_HOST_OPERATION: &str = "conduit.host/wrap-quantity@1";

pub fn quantity_info_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::quantity_info_wrap_semantic_contract();
    let target_kind = Some(contract.kind_id.clone());
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from("wrap-quantity-v1"),
            execution_profile_id: ExecutionProfileId::from("conduit.std/wrap-quantity-kernel@1"),
            implementation_id: ImplementationId::from(QUANTITY_INFO_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-std-host/wrap-quantity@1"),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(QUANTITY_INFO_HOST_OPERATION),
                target_kind,
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
                maximum_output_bytes: conduit_semantic_catalog::QUANTITY_INFO_MAXIMUM_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
