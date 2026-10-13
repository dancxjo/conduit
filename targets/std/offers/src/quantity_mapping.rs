//! Hosted realization of the portable Scalar-to-Quantity mapping contract.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId,
};

pub const QUANTITY_MAP_IMPLEMENTATION: &str = "std/kernel-map-quantity@1";
pub const QUANTITY_MAP_HOST_CALL: &str = "conduit.host/map-quantity@1";
pub const DISTANCE_FREQUENCY_MAP_IMPLEMENTATION: &str = "std/kernel-map-distance-frequency@1";

pub fn quantity_map_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::quantity_map_semantic_contract();
    let target_kind = Some(contract.kind_id.clone());
    let maximum_input_bytes =
        conduit_core::intrinsic_numeric_maximum_bytes(contract.inputs[0].value_kind.as_str())
            .expect("mapping has a finite mathematical input profile") as u32;
    let maximum_output_bytes =
        conduit_core::intrinsic_numeric_maximum_bytes(contract.outputs[0].value_kind.as_str())
            .expect("mapping has a finite physical output profile") as u32;
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from("map-quantity-v1"),
            execution_profile_id: ExecutionProfileId::from("conduit.std/map-quantity-kernel@1"),
            implementation_id: ImplementationId::from(QUANTITY_MAP_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-std-host/map-quantity@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(QUANTITY_MAP_HOST_CALL),
                target_kind,
                maximum_in_flight: 1,
                maximum_input_bytes: maximum_input_bytes,
                maximum_output_bytes: maximum_output_bytes,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

pub fn distance_frequency_map_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::distance_frequency_map_contract()
            .into_semantic_contract(conduit_semantic_catalog::DISTANCE_FREQUENCY_MAP_REVISION),
        Back {
            capability_id: CapabilityId::from("map-distance-frequency-v1"),
            execution_profile_id: ExecutionProfileId::from(
                "conduit.std/map-distance-frequency-kernel@1",
            ),
            implementation_id: ImplementationId::from(DISTANCE_FREQUENCY_MAP_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-std-host/map-distance-frequency@1"),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
