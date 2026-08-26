//! Portable bounded JSON encode/decode Kind contracts.

use super::{StandardKindContract, TerminalBehavior};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    port_id, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, ImplementationId,
    ImplementationOffer, KindContractRevision,
};

pub const JSON_ENCODE_STD_IMPLEMENTATION: &str = "std/kernel-json-encode@1";
pub const JSON_DECODE_STD_IMPLEMENTATION: &str = "std/kernel-json-decode@1";
pub const JSON_STD_PROFILE: &str = "std/no-std-bounded-json@1";
pub const JSON_STD_ARTIFACT: &str = "conduit-core/bounded-json@1";
pub const JSON_CONDUITOS_PROFILE: &str = "conduitos/fixed-bounded-json@1";
pub const JSON_ENCODE_CONDUITOS_IMPLEMENTATION: &str = "conduitos/kernel-json-encode@1";
pub const JSON_DECODE_CONDUITOS_IMPLEMENTATION: &str = "conduitos/kernel-json-decode@1";
pub const JSON_ENCODE_HOST_OPERATION: &str = "conduit.host/json-encode@1";
pub const JSON_DECODE_HOST_OPERATION: &str = "conduit.host/json-decode@1";

pub fn json_encode_contract() -> StandardKindContract {
    contract(
        conduit_web::json_encode_semantics(),
        "JSON encode",
        "Encode one canonical finite JSON value as deterministic bounded UTF-8 text.",
    )
}

pub fn json_decode_contract() -> StandardKindContract {
    contract(
        conduit_web::json_decode_semantics(),
        "JSON decode",
        "Decode one bounded UTF-8 JSON document into canonical finite JSON value semantics.",
    )
}

pub fn json_encode_std_offer() -> CapabilityOffer {
    offer(
        &json_encode_contract(),
        "std-json-encode-v1",
        conduit_web::JSON_ENCODE_REVISION,
        JSON_STD_PROFILE,
        JSON_ENCODE_STD_IMPLEMENTATION,
    )
}

pub fn json_decode_std_offer() -> CapabilityOffer {
    offer(
        &json_decode_contract(),
        "std-json-decode-v1",
        conduit_web::JSON_DECODE_REVISION,
        JSON_STD_PROFILE,
        JSON_DECODE_STD_IMPLEMENTATION,
    )
}

pub fn json_encode_conduitos_offer() -> CapabilityOffer {
    offer(
        &json_encode_contract(),
        "conduitos-json-encode-v1",
        conduit_web::JSON_ENCODE_REVISION,
        JSON_CONDUITOS_PROFILE,
        JSON_ENCODE_CONDUITOS_IMPLEMENTATION,
    )
}

pub fn json_decode_conduitos_offer() -> CapabilityOffer {
    offer(
        &json_decode_contract(),
        "conduitos-json-decode-v1",
        conduit_web::JSON_DECODE_REVISION,
        JSON_CONDUITOS_PROFILE,
        JSON_DECODE_CONDUITOS_IMPLEMENTATION,
    )
}

fn contract(
    contract: conduit_web::PortableKindContract,
    name: &str,
    summary: &str,
) -> StandardKindContract {
    let example = alloc::format!("codec: {}", contract.kind_id.as_str());
    StandardKindContract {
        kind_id: contract.kind_id,
        plain_name: name.to_string(),
        summary: summary.to_string(),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: Vec::new(),
        limits: contract.limits,
        terminal_behavior: TerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example,
    }
}

fn offer(
    contract: &StandardKindContract,
    capability: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
) -> CapabilityOffer {
    let operation = if contract.kind_id.as_str() == conduit_web::JSON_ENCODE_KIND {
        JSON_ENCODE_HOST_OPERATION
    } else {
        JSON_DECODE_HOST_OPERATION
    };
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: Some((port_id("value"), port_id("value"))),
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(revision),
        inputs: contract.inputs.clone(),
        outputs: contract.outputs.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(JSON_STD_ARTIFACT),
        },
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(operation),
            target_kind: Some(contract.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32,
            maximum_output_bytes: conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_are_exact_bounded_portable_and_authority_free() {
        let portable_encode = conduit_web::json_encode_semantics();
        let portable_decode = conduit_web::json_decode_semantics();
        let described_encode = json_encode_contract();
        let described_decode = json_decode_contract();
        let encode = json_encode_std_offer();
        let decode = json_decode_conduitos_offer();
        assert_eq!(described_encode.kind_id, portable_encode.kind_id);
        assert_eq!(described_encode.inputs, portable_encode.inputs);
        assert_eq!(described_encode.outputs, portable_encode.outputs);
        assert_eq!(described_encode.limits, portable_encode.limits);
        assert_eq!(described_decode.kind_id, portable_decode.kind_id);
        assert_eq!(described_decode.inputs, portable_decode.inputs);
        assert_eq!(described_decode.outputs, portable_decode.outputs);
        assert_eq!(described_decode.limits, portable_decode.limits);
        assert_eq!(
            encode.inputs[0].value_kind.as_str(),
            conduit_core::JSON_INFO_ID
        );
        assert_eq!(encode.outputs[0].value_kind, decode.inputs[0].value_kind);
        assert_eq!(
            encode.limits.max_queue_bytes,
            conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32
        );
        assert!(encode.authority_requirements.is_empty());
        assert!(decode.authority_requirements.is_empty());
        assert_ne!(
            encode.implementation.execution_profile_id,
            decode.implementation.execution_profile_id
        );
    }
}
