//! Checked Source ordering; no physical endpoint or authority claim.
use conduit_core::*;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};

fn programs<const N: usize>(entries: [&str; N]) -> [PortableExpressionProgram; N] {
    // These operations belong to one exact inert package. Check it once;
    // each entry still expands independently and owns its prepared evaluator.
    let package = conduitos::protocol_source::usb_hid_endpoint_package().unwrap();
    let source = conduitos::protocol_source::PreparedProtocolSource::prepare(package).unwrap();
    entries.map(|entry| {
        let expanded = source
            .expand(entry)
            .unwrap_or_else(|error| panic!("{entry}: {error:?}"))
            .expanded;
        assert_eq!(expanded.gears.len(), 1);
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!("pure Source expression")
        };
        PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
    })
}

fn field_type(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn insertion(ty: &StructuredInfoType, state: &[u8], ordinal: u64) -> Vec<u8> {
    let observation_type = field_type(ty, "observation");
    let result_type = field_type(&observation_type, "observed");
    let unit = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/unit")).unwrap(),
        vec![],
    )
    .unwrap();
    let observed = StructuredInfoValue::variant(result_type, "short", unit).unwrap();
    let observation = StructuredInfoValue::record(
        observation_type.clone(),
        vec![
            StructuredFieldValue::new(
                "ordinal",
                StructuredInfoValue::leaf(
                    field_type(&observation_type, "ordinal"),
                    ordinal.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new("observed", observed).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new(
                "state",
                StructuredInfoValue::from_canonical_bytes(state).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new("observation", observation).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn encoded(value: ValidatedCanonicalStructuredValue<'_>) -> Vec<u8> {
    let mut bytes = value.type_bytes().to_vec();
    bytes.extend_from_slice(value.value_node());
    bytes
}
fn payload(bytes: &[u8], tag: &str) -> Vec<u8> {
    encoded(
        validate_canonical_structured_value(bytes)
            .unwrap()
            .variant_payload(tag)
            .unwrap()
            .unwrap(),
    )
}

#[test]
fn eight_observations_restore_wire_order_and_refuse_duplicate_stale_and_distant_values() {
    let [initialize, insert, drain] = programs([
        "usb-hid-keyboard-order-initialize",
        "usb-hid-keyboard-order-insert",
        "usb-hid-keyboard-order-drain",
    ]);
    let mut state = initialize.evaluate(&[]).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&drain).unwrap();
    for ordinal in [7, 3, 0, 6, 1, 5, 2, 4] {
        state = payload(
            &insert
                .evaluate(&insertion(&insert.input_type, &state, ordinal))
                .unwrap(),
            "accepted",
        );
    }
    for (ordinal, tag) in [
        (3, "duplicate"),
        (8, "outside-window"),
        (u64::MAX, "outside-window"),
    ] {
        let result = insert
            .evaluate(&insertion(&insert.input_type, &state, ordinal))
            .unwrap();
        super::common::tag(&result, tag);
    }
    for ordinal in 0_u64..8 {
        let allocations = crate::allocation::allocations(|| {
            let result = prepared.evaluate(&state).unwrap();
            let ready = validate_canonical_structured_value(result)
                .unwrap()
                .variant_payload("ready")
                .unwrap()
                .unwrap();
            let observation = ready.record_field("observation").unwrap().unwrap();
            assert_eq!(
                observation
                    .record_field("ordinal")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                ordinal.to_le_bytes()
            );
            assert!(
                observation
                    .record_field("observed")
                    .unwrap()
                    .unwrap()
                    .variant_payload("short")
                    .unwrap()
                    .is_some()
            );
        });
        assert_eq!(allocations, 0);
        let result = drain.evaluate(&state).unwrap();
        let ready = validate_canonical_structured_value(&result)
            .unwrap()
            .variant_payload("ready")
            .unwrap()
            .unwrap();
        state = encoded(ready.record_field("state").unwrap().unwrap());
    }
    super::common::tag(&drain.evaluate(&state).unwrap(), "waiting");
    let stale = insert
        .evaluate(&insertion(&insert.input_type, &state, 7))
        .unwrap();
    super::common::tag(&stale, "stale");
}
