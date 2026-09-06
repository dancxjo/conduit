//! Portable bounded JSON encode/decode Kind contracts.

use super::{StandardKindContract, TerminalBehavior};
use alloc::string::ToString;
use alloc::vec::Vec;

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

pub fn json_collection_step_contract() -> StandardKindContract {
    contract(
        conduit_web::json_collection_step_semantics(),
        "Bounded collection transition",
        "Apply one explicit edit to a bounded JSON array, preserving order and refusing invalid commands.",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_are_exact_bounded_portable_and_authority_free() {
        let portable_encode = conduit_web::json_encode_semantics();
        let portable_decode = conduit_web::json_decode_semantics();
        let described_encode = json_encode_contract();
        let described_decode = json_decode_contract();
        assert_eq!(described_encode.kind_id, portable_encode.kind_id);
        assert_eq!(described_encode.inputs, portable_encode.inputs);
        assert_eq!(described_encode.outputs, portable_encode.outputs);
        assert_eq!(described_encode.limits, portable_encode.limits);
        assert_eq!(described_decode.kind_id, portable_decode.kind_id);
        assert_eq!(described_decode.inputs, portable_decode.inputs);
        assert_eq!(described_decode.outputs, portable_decode.outputs);
        assert_eq!(described_decode.limits, portable_decode.limits);
        assert_eq!(
            described_encode.inputs[0].value_kind.as_str(),
            conduit_web::JSON_INFO_ID
        );
        assert_eq!(
            described_encode.outputs[0].value_kind,
            described_decode.inputs[0].value_kind
        );
        assert_eq!(
            described_encode.limits.max_queue_bytes,
            conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32
        );
    }
}
