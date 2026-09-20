//! Hosted realization of the portable Scalar-to-Quantity mapping contract.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder, CapabilityRealization,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    QUANTITY_ENCODED_LEN, SCALAR_ENCODED_LEN,
};

pub const QUANTITY_MAP_IMPLEMENTATION: &str = "std/kernel-map-quantity@1";
pub const QUANTITY_MAP_HOST_OPERATION: &str = "conduit.host/map-quantity@1";

pub fn quantity_map_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::quantity_map_semantic_contract();
    let target_kind = Some(contract.kind_id.clone());
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from("map-quantity-v1"),
            execution_profile_id: ExecutionProfileId::from("conduit.std/map-quantity-kernel@1"),
            implementation_id: ImplementationId::from(QUANTITY_MAP_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-std-host/map-quantity@1"),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(QUANTITY_MAP_HOST_OPERATION),
                target_kind,
                maximum_in_flight: 1,
                maximum_input_bytes: SCALAR_ENCODED_LEN as u32,
                maximum_output_bytes: QUANTITY_ENCODED_LEN as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
