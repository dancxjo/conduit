//! Browser realizations of the shared bounded JSON semantics.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{CapabilityOffer, ConfigurationValue, HostOperationRequirement, PlannedGear};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};
use conduit_web::JsonValue;

pub(crate) const OPERATIONS: [&str; 4] = [
    "conduit.host/browser-json-encode@1",
    "conduit.host/browser-json-decode@1",
    "conduit.host/browser-json-collection-step@1",
    "conduit.host/browser-json-boolean-summary@1",
];
const IMPLEMENTATIONS: [&str; 4] = [
    "browser/kernel-json-encode@1",
    "browser/kernel-json-decode@1",
    "browser/kernel-json-collection-step@1",
    "browser/kernel-json-boolean-summary@1",
];
const MAXIMUM: u32 = conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32;

pub(super) static ENCODE: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATIONS[0],
    offer: encode_offer,
    prepare,
    perform: None,
};
pub(super) static DECODE: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATIONS[1],
    offer: decode_offer,
    prepare,
    perform: None,
};
pub(super) static COLLECTION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATIONS[2],
    offer: collection_offer,
    prepare,
    perform: None,
};
pub(super) static SUMMARY: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATIONS[3],
    offer: summary_offer,
    prepare,
    perform: None,
};
fn encode_offer() -> CapabilityOffer {
    offer(0)
}
fn decode_offer() -> CapabilityOffer {
    offer(1)
}
fn collection_offer() -> CapabilityOffer {
    offer(2)
}
fn summary_offer() -> CapabilityOffer {
    offer(3)
}

fn offer(index: usize) -> CapabilityOffer {
    let (contract, revision) = match index {
        0 => (
            conduit_semantic_catalog::json_encode_contract(),
            conduit_web::JSON_ENCODE_REVISION,
        ),
        1 => (
            conduit_semantic_catalog::json_decode_contract(),
            conduit_web::JSON_DECODE_REVISION,
        ),
        2 => (
            conduit_semantic_catalog::json_collection_step_contract(),
            conduit_web::JSON_COLLECTION_STEP_REVISION,
        ),
        _ => (
            conduit_semantic_catalog::json_boolean_summary_contract(),
            conduit_web::JSON_BOOLEAN_SUMMARY_REVISION,
        ),
    };
    let kind = contract.kind_id.clone();
    conduit_semantic_catalog::realization_offer(
        contract,
        revision,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATIONS[index],
            execution_profile: "browser/bounded-json@1",
            implementation: IMPLEMENTATIONS[index],
            artifact: "conduit-browser-runtime/bounded-json@1",
        },
        vec![HostOperationRequirement {
            contract_id: OPERATIONS[index].into(),
            target_kind: Some(kind),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM,
            maximum_output_bytes: MAXIMUM,
        }],
        Vec::new(),
        Vec::new(),
    )
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    let index = IMPLEMENTATIONS
        .iter()
        .position(|id| *id == placement.implementation_id.as_str())
        .ok_or("unknown browser JSON implementation")?;
    validate_placement(placement, &offer(index))?;
    if index == 3 {
        field(placement)?;
    } else if !placement.configuration.is_empty() {
        return Err("unexpected JSON configuration".into());
    }
    Ok(BrowserOperation::unary(MAXIMUM, 4))
}

fn field(placement: &PlannedGear) -> Result<&str, String> {
    match placement.configuration.as_slice() {
        [entry] if entry.key == "field" => match &entry.value {
            ConfigurationValue::Text(value)
                if !value.is_empty() && value.len() <= conduit_web::JSON_MAXIMUM_KEY_BYTES =>
            {
                Ok(value)
            }
            _ => Err("JSON summary requires a bounded nonempty field".into()),
        },
        _ => Err("JSON summary requires exactly one field".into()),
    }
}

pub(crate) fn execute(
    placement: &PlannedGear,
    contract: &str,
    input: &[u8],
) -> Result<Vec<u8>, Failure> {
    let result: Result<Vec<u8>, u16> = match contract {
        value if value == OPERATIONS[0] => JsonValue::decode_info(input)
            .and_then(|value| value.encode_text())
            .map_err(|e| e as u16),
        value if value == OPERATIONS[1] => JsonValue::decode_text(input)
            .and_then(|value| value.encode_info())
            .map_err(|e| e as u16),
        value if value == OPERATIONS[2] => {
            conduit_web::json_collection_step_bytes(input).map_err(collection_detail)
        }
        value if value == OPERATIONS[3] => (|| {
            let value = JsonValue::decode_info(input).map_err(|e| e as u16)?;
            let field = field(placement).map_err(|_| 120_u16)?;
            conduit_web::json_boolean_summary(&value, field)
                .map_err(|e| e.detail())?
                .encode_info()
                .map_err(|e| e as u16)
        })(),
        _ => Err(125),
    };
    result.map_err(|detail| Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

fn collection_detail(error: conduit_web::JsonCollectionRefusal) -> u16 {
    use conduit_web::JsonCollectionRefusal::*;
    match error {
        InvalidRequest => 100,
        InvalidCollection => 101,
        InvalidCommand => 102,
        UnknownOperation => 103,
        InvalidIndex => 104,
        MissingIndex => 105,
        MissingField => 106,
        NotBoolean => 107,
        CollectionFull => 108,
        InvalidValue(error) => error as u16,
    }
}
