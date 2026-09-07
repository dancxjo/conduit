//! Browser realizations of the shared, transport-neutral typed-record codecs.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, PlannedGear, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const OPERATIONS: [&str; 4] = [
    "conduit.host/browser-text-to-typed-record@1",
    "conduit.host/browser-typed-record-frame@1",
    "conduit.host/browser-typed-record-deframe@1",
    "conduit.host/browser-typed-record-to-text@1",
];
const IMPLEMENTATIONS: [&str; 4] = [
    "browser/text-to-typed-record@1",
    "browser/typed-record-frame@1",
    "browser/typed-record-deframe@1",
    "browser/typed-record-to-text@1",
];
const MAXIMUM: u32 = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;

pub(super) static TEXT_TO_RECORD: BrowserInstallation = installation(0, text_to_record_offer);
pub(super) static FRAME: BrowserInstallation = installation(1, frame_offer);
pub(super) static DEFRAME: BrowserInstallation = installation(2, deframe_offer);
pub(super) static RECORD_TO_TEXT: BrowserInstallation = installation(3, record_to_text_offer);

const fn installation(index: usize, offer: fn() -> CapabilityOffer) -> BrowserInstallation {
    BrowserInstallation {
        implementation_id: IMPLEMENTATIONS[index],
        offer,
        prepare,
        perform: None,
    }
}

fn text_to_record_offer() -> CapabilityOffer {
    offer(0)
}
fn frame_offer() -> CapabilityOffer {
    offer(1)
}
fn deframe_offer() -> CapabilityOffer {
    offer(2)
}
fn record_to_text_offer() -> CapabilityOffer {
    offer(3)
}

fn offer(index: usize) -> CapabilityOffer {
    let (kind, revision) = match index {
        0 => (
            conduit_net::TEXT_TO_TYPED_RECORD_KIND,
            conduit_net::TEXT_RECORD_CONTRACT_REVISION,
        ),
        1 => (
            conduit_net::TYPED_RECORD_FRAME_KIND,
            conduit_net::TYPED_RECORD_CONTRACT_REVISION,
        ),
        2 => (
            conduit_net::TYPED_RECORD_DEFRAME_KIND,
            conduit_net::TYPED_RECORD_CONTRACT_REVISION,
        ),
        _ => (
            conduit_net::TYPED_RECORD_TO_TEXT_KIND,
            conduit_net::TEXT_RECORD_CONTRACT_REVISION,
        ),
    };
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile)
        .expect("typed-record catalog is exact");
    let definition = profile
        .get(&conduit_core::kind_id(kind))
        .expect("typed-record codec definition exists");
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATIONS[index]),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/typed-record-codec@1"),
            implementation_id: ImplementationId::from(IMPLEMENTATIONS[index]),
            artifact_id: ArtifactId::from("conduit-browser-runtime/typed-record-codecs@1"),
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(OPERATIONS[index]),
            target_kind: Some(definition.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM,
            maximum_output_bytes: MAXIMUM,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: MAXIMUM * 4,
        },
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    let index = IMPLEMENTATIONS
        .iter()
        .position(|id| *id == placement.implementation_id.as_str())
        .ok_or("unknown browser typed-record implementation")?;
    validate_placement(placement, &offer(index))?;
    if !placement.configuration.is_empty() {
        return Err("unexpected typed-record codec configuration".into());
    }
    Ok(BrowserOperation::unary(MAXIMUM, 4))
}

pub(crate) fn execute(contract: &str, input: &[u8]) -> Result<Vec<u8>, Failure> {
    execute_inner(contract, input).map_err(|detail| Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

fn execute_inner(contract: &str, input: &[u8]) -> Result<Vec<u8>, u16> {
    match contract {
        operation if operation == OPERATIONS[0] => {
            let text = StructuredInfoValue::leaf(conduit_net::text_type(), input.to_vec())
                .map_err(|_| 1_u16)?;
            canonical(conduit_net::typed_record_from_text(&text).map_err(text_detail)?)
        }
        operation if operation == OPERATIONS[1] => {
            let record = decode(input)?;
            let mut frame = vec![0; conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES];
            let length = conduit_net::frame_typed_record_value_into(&record, &mut frame)
                .map_err(frame_detail)?;
            frame.truncate(length);
            canonical(conduit_net::framed_typed_record_value(&frame).map_err(frame_detail)?)
        }
        operation if operation == OPERATIONS[2] => {
            let framed = decode(input)?;
            canonical(conduit_net::deframe_typed_record_value(&framed).map_err(frame_detail)?)
        }
        operation if operation == OPERATIONS[3] => {
            let record = decode(input)?;
            let text = conduit_net::text_from_typed_record(&record).map_err(text_detail)?;
            let StructuredInfoValueShape::Leaf(bytes) = text.shape() else {
                return Err(2_u16);
            };
            Ok(bytes.to_vec())
        }
        _ => Err(3_u16),
    }
}

fn decode(input: &[u8]) -> Result<StructuredInfoValue, u16> {
    StructuredInfoValue::from_canonical_bytes(input).map_err(|_| 4_u16)
}

fn canonical(value: StructuredInfoValue) -> Result<Vec<u8>, u16> {
    value.canonical_bytes().map_err(|_| 5_u16)
}

fn text_detail(error: conduit_net::TextRecordRefusal) -> u16 {
    match error {
        conduit_net::TextRecordRefusal::WrongTextValueType => 10,
        conduit_net::TextRecordRefusal::WrongTypedRecordValueType => 11,
        conduit_net::TextRecordRefusal::TypedRecord(error) => frame_detail(error),
    }
}

fn frame_detail(error: conduit_net::TypedRecordFrameRefusal) -> u16 {
    use conduit_net::TypedRecordFrameRefusal::*;
    match error {
        MissingValueKind => 20,
        ValueKindTooLarge => 21,
        InvalidValueKindEncoding => 22,
        PayloadTooLarge => 23,
        FrameTooLarge => 24,
        MalformedPayload => 25,
        PayloadTypeMismatch => 26,
        OutputTooSmall => 27,
        Truncated => 28,
        InvalidMagic => 29,
        UnsupportedVersion => 30,
        LengthOverflow => 31,
        TrailingBytes => 32,
        IntegrityMismatch => 33,
        WrongTypedRecordValueType => 34,
        WrongFramedTypedRecordValueType => 35,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_codec_family_round_trips_exact_text() {
        let record = execute(OPERATIONS[0], b"CALLING").unwrap();
        let frame = execute(OPERATIONS[1], &record).unwrap();
        let recovered = execute(OPERATIONS[2], &frame).unwrap();
        assert_eq!(execute(OPERATIONS[3], &recovered).unwrap(), b"CALLING");
    }

    #[test]
    fn record_to_text_refuses_a_non_record_value() {
        let text = StructuredInfoValue::leaf(conduit_net::text_type(), b"CALLING".to_vec())
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert_eq!(
            execute(OPERATIONS[3], &text).unwrap_err().code,
            FailureCode::InvalidInput
        );
    }
}
