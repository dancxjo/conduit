//! Source pointer-history projection, separate from ordinal storage proof.
use super::{allocation, common::*};
use conduit_core::*;

#[test]
fn pointer_history_preserves_normalized_motion_bounds_invalid_reports_and_sequence() {
    let [initialize, advance] = programs([
        "usb-hid-mouse-pointer-initialize",
        "usb-hid-mouse-pointer-advance",
    ]);
    let mut state = initialize.evaluate(&[]).unwrap();
    let input_type = &advance.input_type;
    let observation_type = field_type(input_type, "observation");
    let observed_type = field_type(&observation_type, "observed");
    let StructuredInfoTypeShape::Variant { cases, .. } = observed_type.shape() else {
        panic!("observed")
    };
    let motion_type = cases
        .iter()
        .find(|case| case.tag() == "mouse")
        .unwrap()
        .payload_type();
    let input = |state: &[u8], ordinal: u64, motion: Option<(u8, i16, i16)>, capacity: u64| {
        let observed = match motion {
            Some((buttons, x, y)) => StructuredInfoValue::variant(
                observed_type.clone(),
                "mouse",
                StructuredInfoValue::record(
                    motion_type.clone(),
                    vec![
                        leaf_field(motion_type, "buttons", &[buttons]),
                        leaf_field(motion_type, "x", &x.to_le_bytes()),
                        leaf_field(motion_type, "y", &y.to_le_bytes()),
                    ],
                )
                .unwrap(),
            )
            .unwrap(),
            None => StructuredInfoValue::variant(
                observed_type.clone(),
                "short",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id("value/unit")).unwrap(),
                    vec![],
                )
                .unwrap(),
            )
            .unwrap(),
        };
        StructuredInfoValue::record(
            input_type.clone(),
            vec![
                StructuredFieldValue::new(
                    "state",
                    StructuredInfoValue::from_canonical_bytes(state).unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "observation",
                    StructuredInfoValue::record(
                        observation_type.clone(),
                        vec![
                            leaf_field(&observation_type, "ordinal", &ordinal.to_le_bytes()),
                            StructuredFieldValue::new("observed", observed).unwrap(),
                        ],
                    )
                    .unwrap(),
                )
                .unwrap(),
                leaf_field(input_type, "queue-capacity", &capacity.to_le_bytes()),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    };
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&advance).unwrap();
    for (ordinal, buttons, x, y, position_x, position_y) in [
        (0_u64, 7_u8, 127_i16, -127_i16, 1_000_000_i64, 0_i64),
        (1, 0, -127, 127, 492_000, 508_000),
    ] {
        let bytes = input(&state, ordinal, Some((buttons, x, y)), 8);
        let allocations = allocation::allocations(|| {
            let value =
                validate_canonical_structured_value(prepared.evaluate(&bytes).unwrap()).unwrap();
            let ready = value.variant_payload("ready").unwrap().unwrap();
            let sample = ready.record_field("sample").unwrap().unwrap();
            for (name, expected) in [
                ("position-x", position_x),
                ("position-y", position_y),
                ("delta-x", i64::from(x) * 4000),
                ("delta-y", i64::from(y) * 4000),
            ] {
                assert_eq!(
                    sample
                        .record_field(name)
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/i64")
                        .unwrap(),
                    expected.to_le_bytes()
                );
            }
            assert_eq!(
                sample
                    .record_field("primary-pressed")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bool")
                    .unwrap(),
                [u8::from(buttons & 1 != 0)]
            );
            for name in ["coalesced", "dropped"] {
                assert_eq!(
                    sample
                        .record_field(name)
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    0_u64.to_le_bytes()
                );
            }
            assert_eq!(
                sample
                    .record_field("sequence")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                (ordinal + 1).to_le_bytes()
            );
            assert_eq!(
                sample
                    .record_field("queue-capacity")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                8_u64.to_le_bytes()
            );
        });
        assert_eq!(allocations, 0);
        let output = advance.evaluate(&bytes).unwrap();
        let ready = validate_canonical_structured_value(&output)
            .unwrap()
            .variant_payload("ready")
            .unwrap()
            .unwrap();
        let sample_bytes = encoded(ready.record_field("sample").unwrap().unwrap());
        let sample_value = StructuredInfoValue::from_canonical_bytes(&sample_bytes).unwrap();
        let decoder = conduitos::source_pointer_sample::SourcePointerSampleDecoder::prepare(
            sample_value.value_type(),
        )
        .unwrap();
        let allocations = allocation::allocations(|| {
            let sample = decoder.decode(&sample_bytes).unwrap();
            assert_eq!(sample.position_x, position_x);
            assert_eq!(sample.position_y, position_y);
            assert_eq!(sample.delta_x, i64::from(x) * 4000);
            assert_eq!(sample.delta_y, i64::from(y) * 4000);
            assert_eq!(sample.primary_pressed, buttons & 1 != 0);
            assert_eq!(
                (
                    sample.coalesced,
                    sample.dropped,
                    sample.queue_capacity,
                    sample.sequence
                ),
                (0, 0, 8, ordinal + 1)
            );
        });
        assert_eq!(allocations, 0);
        assert!(
            conduit_semantic_catalog::normalized_pointer_value(
                decoder.decode(&sample_bytes).unwrap()
            )
            .is_ok()
        );
        assert!(
            decoder.decode(&state).is_err(),
            "a history value is not a sample"
        );
        assert!(
            decoder
                .decode(&sample_bytes[..sample_bytes.len() - 1])
                .is_err()
        );
        state = encoded(
            validate_canonical_structured_value(&output)
                .unwrap()
                .variant_payload("ready")
                .unwrap()
                .unwrap()
                .record_field("state")
                .unwrap()
                .unwrap(),
        );
    }
    let skipped = advance.evaluate(&input(&state, 2, None, 8)).unwrap();
    let retained = encoded(
        validate_canonical_structured_value(&skipped)
            .unwrap()
            .variant_payload("skipped")
            .unwrap()
            .unwrap()
            .record_field("state")
            .unwrap()
            .unwrap(),
    );
    assert_eq!(retained, state);
    assert_tag(
        &advance
            .evaluate(&input(&state, 3, Some((0, -128, 0)), 8))
            .unwrap(),
        "invalid-motion",
    );
    assert_tag(
        &advance
            .evaluate(&input(&state, 3, Some((8, 0, 0)), 8))
            .unwrap(),
        "invalid-motion",
    );
    assert_tag(
        &advance
            .evaluate(&input(&state, 3, Some((0, 0, 0)), 0))
            .unwrap(),
        "invalid-state",
    );
    let state_type = field_type(input_type, "state");
    let exhausted = StructuredInfoValue::record(
        state_type.clone(),
        vec![
            leaf_field(&state_type, "position-x", &500000_i64.to_le_bytes()),
            leaf_field(&state_type, "position-y", &500000_i64.to_le_bytes()),
            leaf_field(&state_type, "sequence", &u64::MAX.to_le_bytes()),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert_tag(
        &advance
            .evaluate(&input(&exhausted, 4, Some((0, 0, 0)), 8))
            .unwrap(),
        "sequence-exhausted",
    );
}
