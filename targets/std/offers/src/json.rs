//! Exact bounded JSON realization offers owned by the hosted std Host.

use conduit_core::{CapabilityOffer, HostOperationContractId, HostOperationRequirement};

pub const JSON_ENCODE_STD_IMPLEMENTATION: &str = "std/kernel-json-encode@1";
pub const JSON_DECODE_STD_IMPLEMENTATION: &str = "std/kernel-json-decode@1";
pub const JSON_ENCODE_HOST_OPERATION: &str = "conduit.host/json-encode@1";
pub const JSON_DECODE_HOST_OPERATION: &str = "conduit.host/json-decode@1";
pub const JSON_COLLECTION_STEP_STD_IMPLEMENTATION: &str = "std/kernel-json-collection-step@1";
pub const JSON_COLLECTION_STEP_HOST_OPERATION: &str = "conduit.host/json-collection-step@1";

pub fn json_collection_step_std_offer() -> CapabilityOffer {
    json_offer(
        conduit_semantic_catalog::json_collection_step_contract(),
        conduit_web::JSON_COLLECTION_STEP_REVISION,
        "std-json-collection-step-v1",
        JSON_COLLECTION_STEP_STD_IMPLEMENTATION,
        JSON_COLLECTION_STEP_HOST_OPERATION,
    )
}

pub fn json_encode_std_offer() -> CapabilityOffer {
    json_offer(
        conduit_semantic_catalog::json_encode_contract(),
        conduit_web::JSON_ENCODE_REVISION,
        "std-json-encode-v1",
        JSON_ENCODE_STD_IMPLEMENTATION,
        JSON_ENCODE_HOST_OPERATION,
    )
}

pub fn json_decode_std_offer() -> CapabilityOffer {
    json_offer(
        conduit_semantic_catalog::json_decode_contract(),
        conduit_web::JSON_DECODE_REVISION,
        "std-json-decode-v1",
        JSON_DECODE_STD_IMPLEMENTATION,
        JSON_DECODE_HOST_OPERATION,
    )
}

fn json_offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    revision: &str,
    capability: &str,
    implementation: &str,
    operation: &str,
) -> CapabilityOffer {
    let target_kind = contract.kind_id.clone();
    let mut offer = conduit_semantic_catalog::realization_offer(
        contract,
        revision,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability,
            execution_profile: "std/no-std-bounded-json@1",
            implementation,
            artifact: "conduit-core/bounded-json@1",
        },
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(operation),
            target_kind: Some(target_kind),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
            maximum_output_bytes: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
        }],
        Vec::new(),
        Vec::new(),
    );
    offer.shorthand = Some((
        conduit_core::port_id("value"),
        conduit_core::port_id("value"),
    ));
    offer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_preserve_portable_contracts_and_distinct_realizations() {
        let encode = json_encode_std_offer();
        let decode = json_decode_std_offer();
        assert_eq!(
            encode.kind_id,
            conduit_semantic_catalog::json_encode_contract().kind_id
        );
        assert_eq!(
            decode.kind_id,
            conduit_semantic_catalog::json_decode_contract().kind_id
        );
        assert!(encode.authority_requirements.is_empty());
        assert!(decode.authority_requirements.is_empty());
        assert_ne!(
            encode.implementation.implementation_id,
            decode.implementation.implementation_id
        );
    }
}
