//! Hosted realization of the portable Scalar-to-Quantity mapping contract.

use conduit_core::{
    CapabilityOffer, HostOperationContractId, HostOperationRequirement, QUANTITY_ENCODED_LEN,
    SCALAR_ENCODED_LEN,
};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};

pub const QUANTITY_MAP_IMPLEMENTATION: &str = "std/kernel-map-quantity@1";
pub const QUANTITY_MAP_HOST_OPERATION: &str = "conduit.host/map-quantity@1";

pub fn quantity_map_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::quantity_map_contract();
    let target_kind = Some(contract.kind_id.clone());
    realization_offer(
        contract,
        conduit_semantic_catalog::QUANTITY_MAP_REVISION,
        RealizationOfferIdentity {
            capability: "map-quantity-v1",
            execution_profile: "conduit.std/map-quantity-kernel@1",
            implementation: QUANTITY_MAP_IMPLEMENTATION,
            artifact: "conduit-std-host/map-quantity@1",
        },
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(QUANTITY_MAP_HOST_OPERATION),
            target_kind,
            maximum_in_flight: 1,
            maximum_input_bytes: SCALAR_ENCODED_LEN as u32,
            maximum_output_bytes: QUANTITY_ENCODED_LEN as u32,
        }],
        Vec::new(),
        Vec::new(),
    )
}
