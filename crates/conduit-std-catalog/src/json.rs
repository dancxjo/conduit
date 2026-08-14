//! Portable bounded JSON encode/decode Kind contracts.

use super::{StandardKindContract, TerminalBehavior};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal,
};

pub const JSON_ENCODE_KIND: &str = "json/encode";
pub const JSON_DECODE_KIND: &str = "json/decode";
pub const JSON_ENCODE_REVISION: &str = "conduit.std/json-encode@1";
pub const JSON_DECODE_REVISION: &str = "conduit.std/json-decode@1";
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
        JSON_ENCODE_KIND,
        "JSON encode",
        "Encode one canonical finite JSON value as deterministic bounded UTF-8 text.",
        conduit_core::JSON_INFO_ID,
        conduit_core::JSON_TEXT_INFO_ID,
    )
}

pub fn json_decode_contract() -> StandardKindContract {
    contract(
        JSON_DECODE_KIND,
        "JSON decode",
        "Decode one bounded UTF-8 JSON document into canonical finite JSON value semantics.",
        conduit_core::JSON_TEXT_INFO_ID,
        conduit_core::JSON_INFO_ID,
    )
}

pub fn json_encode_std_offer() -> CapabilityOffer {
    offer(
        &json_encode_contract(),
        "std-json-encode-v1",
        JSON_ENCODE_REVISION,
        JSON_STD_PROFILE,
        JSON_ENCODE_STD_IMPLEMENTATION,
    )
}

pub fn json_decode_std_offer() -> CapabilityOffer {
    offer(
        &json_decode_contract(),
        "std-json-decode-v1",
        JSON_DECODE_REVISION,
        JSON_STD_PROFILE,
        JSON_DECODE_STD_IMPLEMENTATION,
    )
}

pub fn json_encode_conduitos_offer() -> CapabilityOffer {
    offer(
        &json_encode_contract(),
        "conduitos-json-encode-v1",
        JSON_ENCODE_REVISION,
        JSON_CONDUITOS_PROFILE,
        JSON_ENCODE_CONDUITOS_IMPLEMENTATION,
    )
}

pub fn json_decode_conduitos_offer() -> CapabilityOffer {
    offer(
        &json_decode_contract(),
        "conduitos-json-decode-v1",
        JSON_DECODE_REVISION,
        JSON_CONDUITOS_PROFILE,
        JSON_DECODE_CONDUITOS_IMPLEMENTATION,
    )
}

#[cfg(feature = "form-catalog")]
pub fn install_json_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{KindDefinition, KindSignature};
    for (contract, revision) in [
        (json_encode_contract(), JSON_ENCODE_REVISION),
        (json_decode_contract(), JSON_DECODE_REVISION),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn contract(
    kind: &str,
    name: &str,
    summary: &str,
    input: &str,
    output: &str,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: name.to_string(),
        summary: summary.to_string(),
        inputs: vec![port("value", input, PortDirection::Input)],
        outputs: vec![port("value", output, PortDirection::Output)],
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 4,
            max_queue_bytes: conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32,
        },
        terminal_behavior: TerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: alloc::format!("codec: {kind}"),
    }
}

fn port(name: &str, value: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn offer(
    contract: &StandardKindContract,
    capability: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
) -> CapabilityOffer {
    let operation = if contract.kind_id.as_str() == JSON_ENCODE_KIND {
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
        let encode = json_encode_std_offer();
        let decode = json_decode_conduitos_offer();
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
