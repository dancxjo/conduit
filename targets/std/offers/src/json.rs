//! Exact bounded JSON realization offers owned by the hosted std Host.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, Kind,
};

pub const JSON_ENCODE_STD_IMPLEMENTATION: &str = "std/kernel-json-encode@1";
pub const JSON_DECODE_STD_IMPLEMENTATION: &str = "std/kernel-json-decode@1";
pub const JSON_ENCODE_HOST_CALL: &str = "conduit.host/json-encode@1";
pub const JSON_DECODE_HOST_CALL: &str = "conduit.host/json-decode@1";
pub const JSON_COLLECTION_STEP_STD_IMPLEMENTATION: &str = "std/kernel-json-collection-step@1";
pub const JSON_COLLECTION_STEP_HOST_CALL: &str = "conduit.host/json-collection-step@1";
pub const JSON_BOOLEAN_SUMMARY_STD_IMPLEMENTATION: &str = "std/kernel-json-boolean-summary@1";
pub const JSON_BOOLEAN_SUMMARY_HOST_CALL: &str = "conduit.host/json-boolean-summary@1";

pub fn json_boolean_summary_std_offer() -> CapabilityOffer {
    json_offer(
        conduit_semantic_catalog::json_boolean_summary_semantic_contract(),
        "std-json-boolean-summary-v1",
        JSON_BOOLEAN_SUMMARY_STD_IMPLEMENTATION,
        JSON_BOOLEAN_SUMMARY_HOST_CALL,
    )
}

pub fn json_collection_step_std_offer() -> CapabilityOffer {
    json_offer(
        conduit_semantic_catalog::json_collection_step_semantic_contract(),
        "std-json-collection-step-v1",
        JSON_COLLECTION_STEP_STD_IMPLEMENTATION,
        JSON_COLLECTION_STEP_HOST_CALL,
    )
}

pub fn json_encode_std_offer() -> CapabilityOffer {
    json_offer(
        conduit_semantic_catalog::json_encode_semantic_contract(),
        "std-json-encode-v1",
        JSON_ENCODE_STD_IMPLEMENTATION,
        JSON_ENCODE_HOST_CALL,
    )
}

pub fn json_decode_std_offer() -> CapabilityOffer {
    json_offer(
        conduit_semantic_catalog::json_decode_semantic_contract(),
        "std-json-decode-v1",
        JSON_DECODE_STD_IMPLEMENTATION,
        JSON_DECODE_HOST_CALL,
    )
}

fn json_offer(
    contract: Kind,
    capability: &str,
    implementation: &str,
    operation: &str,
) -> CapabilityOffer {
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from("std/no-std-bounded-json@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-core/bounded-json@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(operation),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
                maximum_output_bytes: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
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
