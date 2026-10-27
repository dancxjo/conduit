use super::{
    DigitalDisposition, DigitalRequest, contract::DigitalContract, decode::PreparedDigitalDecoder,
    result::PreparedDigitalResultEncoder,
};
use alloc::{vec, vec::Vec};
use conduit_core::{
    StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
    validate_canonical_structured_value,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};
pub(super) fn request(contract: &DigitalContract, request: DigitalRequest) -> Vec<u8> {
    let tag = if matches!(request, DigitalRequest::Observe) {
        "observe"
    } else {
        "drive"
    };
    let StructuredInfoTypeShape::Variant { cases, .. } = contract.request_type().shape() else {
        panic!("request")
    };
    let payload_type = cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type()
        .clone();
    let payload = match request {
        DigitalRequest::Observe => StructuredInfoValue::leaf(payload_type, vec![]).unwrap(),
        DigitalRequest::Drive(high) => {
            let StructuredInfoTypeShape::Record { fields, .. } = payload_type.shape() else {
                panic!("drive");
            };
            let high_type = fields[0].value_type().clone();
            StructuredInfoValue::record(
                payload_type,
                vec![
                    StructuredFieldValue::new(
                        "high",
                        StructuredInfoValue::leaf(high_type, vec![u8::from(high)]).unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
        }
    };
    StructuredInfoValue::variant(contract.request_type().clone(), tag, payload)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
#[test]
fn checked_digital_topology_has_no_pin_or_direction_configuration() {
    let syntax = parse_syntax_document(super::contract::DIGITAL_TYPES);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new()).unwrap();
    let contract = DigitalContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    let syntax = parse_syntax_document(include_str!(
        "../../../../plots/device-protocols/digital.conduit"
    ));
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "digital-access", &profile).unwrap();
    let [gear] = authoring.expanded.gears.as_slice() else {
        panic!("one digital access");
    };
    assert_eq!(gear.kind_id, contract.kind().kind_id);
    assert!(gear.configuration.is_empty());
}
#[test]
fn exact_requests_and_all_results_need_no_runtime_allocation() {
    let contract = DigitalContract::prepare().unwrap();
    let decoder = PreparedDigitalDecoder::new(&contract).unwrap();
    let mut encoder = PreparedDigitalResultEncoder::new(&contract).unwrap();
    for operation in [
        DigitalRequest::Observe,
        DigitalRequest::Drive(false),
        DigitalRequest::Drive(true),
    ] {
        let input = request(&contract, operation);
        for length in 0..input.len() {
            assert!(decoder.decode(&input[..length]).is_err());
        }
        let mut trailing = input.clone();
        trailing.push(0);
        assert!(decoder.decode(&trailing).is_err());
        let allocations = crate::test_allocations::allocations(|| {
            assert_eq!(decoder.decode(&input).unwrap(), operation);
            for high in [false, true] {
                let bytes = encoder.completed(high).unwrap();
                let value = validate_canonical_structured_value(bytes).unwrap();
                assert_eq!(
                    value
                        .variant_payload("completed")
                        .unwrap()
                        .unwrap()
                        .record_field("high")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/bool")
                        .unwrap(),
                    &[u8::from(high)]
                );
            }
            for disposition in [
                DigitalDisposition::ProviderLost,
                DigitalDisposition::Unsupported,
                DigitalDisposition::Refused,
            ] {
                assert_ne!(
                    validate_canonical_structured_value(encoder.disposition(disposition).unwrap())
                        .unwrap()
                        .variant_tag()
                        .unwrap(),
                    "completed"
                );
            }
        });
        assert_eq!(allocations, 0);
    }
}
