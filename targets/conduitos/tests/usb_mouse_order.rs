//! Two-capture mouse ordering in checked Source; no device execution claim.
#[path = "../../../architecture/plot/tests/prepared_structured_payload/allocation.rs"]
mod allocation;
#[path = "usb_mouse_order/common.rs"]
mod common;
use common::*;
use conduit_core::*;
use conduit_plot::PreparedPortableExpressionEvaluator;

#[test]
fn two_captures_preserve_invalid_observations_and_refuse_duplicate_stale_and_distant_values() {
    let [initialize, insert, drain] = programs([
        "usb-hid-mouse-order-initialize",
        "usb-hid-mouse-order-insert",
        "usb-hid-mouse-order-drain",
    ]);
    let mut state = initialize.evaluate(&[]).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&drain).unwrap();
    for ordinal in [1, 0] {
        state = payload(
            &insert
                .evaluate(&insertion(
                    &insert.input_type,
                    &state,
                    ordinal,
                    if ordinal == 0 {
                        "short"
                    } else {
                        "invalid-motion"
                    },
                ))
                .unwrap(),
            "accepted",
        );
    }
    for (ordinal, tag) in [
        (1, "duplicate"),
        (2, "outside-window"),
        (u64::MAX, "outside-window"),
    ] {
        let result = insert
            .evaluate(&insertion(
                &insert.input_type,
                &state,
                ordinal,
                if ordinal == 0 {
                    "short"
                } else {
                    "invalid-motion"
                },
            ))
            .unwrap();
        assert_tag(&result, tag);
    }
    for ordinal in 0_u64..2 {
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
                    .variant_payload(if ordinal == 0 {
                        "short"
                    } else {
                        "invalid-motion"
                    })
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
    assert_tag(&drain.evaluate(&state).unwrap(), "waiting");
    let stale = insert
        .evaluate(&insertion(&insert.input_type, &state, 1, "short"))
        .unwrap();
    assert_tag(&stale, "stale");
}

#[test]
fn compact_observation_preserves_motion_buttons_and_every_invalid_disposition_without_growth() {
    let projection = program("usb-hid-mouse-compact-observation");
    let observed_type = field_type(&projection.input_type, "observed");
    let StructuredInfoTypeShape::Variant { cases, .. } = observed_type.shape() else {
        panic!("mouse result")
    };
    let report_type = cases
        .iter()
        .find(|case| case.tag() == "mouse")
        .unwrap()
        .payload_type();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&projection).unwrap();
    // Exercise the full declared packed-wire bound independently of the
    // upstream report decoder's stricter valid-report extent.
    let wire = [0xa5; 2048];
    for (ordinal, (x, y, buttons)) in [(-127_i16, 127_i16, 7_u8), (0, 0, 0), (127, -127, 1)]
        .into_iter()
        .enumerate()
    {
        let report = StructuredInfoValue::record(
            report_type.clone(),
            vec![
                leaf_field(report_type, "buttons", &[buttons]),
                leaf_field(report_type, "x", &x.to_le_bytes()),
                leaf_field(report_type, "y", &y.to_le_bytes()),
                leaf_field(report_type, "wire", &wire),
            ],
        )
        .unwrap();
        let observation = StructuredInfoValue::record(
            projection.input_type.clone(),
            vec![
                leaf_field(
                    &projection.input_type,
                    "ordinal",
                    &(ordinal as u64).to_le_bytes(),
                ),
                StructuredFieldValue::new(
                    "observed",
                    StructuredInfoValue::variant(observed_type.clone(), "mouse", report).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        assert!(observation.len() <= 4096);
        let allocations = allocation::allocations(|| {
            let bytes = evaluator.evaluate(&observation).unwrap();
            assert!(bytes.len() <= 4096);
            let value = validate_canonical_structured_value(bytes).unwrap();
            assert_eq!(
                value
                    .record_field("ordinal")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                (ordinal as u64).to_le_bytes()
            );
            let motion = value
                .record_field("observed")
                .unwrap()
                .unwrap()
                .variant_payload("mouse")
                .unwrap()
                .unwrap();
            assert_eq!(
                motion
                    .record_field("buttons")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u8")
                    .unwrap(),
                [buttons]
            );
            assert_eq!(
                motion
                    .record_field("x")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/i16")
                    .unwrap(),
                x.to_le_bytes()
            );
            assert_eq!(
                motion
                    .record_field("y")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/i16")
                    .unwrap(),
                y.to_le_bytes()
            );
        });
        assert_eq!(allocations, 0);
    }
    for disposition in ["short", "malformed", "reserved", "invalid-motion"] {
        let unit = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/unit")).unwrap(),
            vec![],
        )
        .unwrap();
        let observation = StructuredInfoValue::record(
            projection.input_type.clone(),
            vec![
                leaf_field(&projection.input_type, "ordinal", &19_u64.to_le_bytes()),
                StructuredFieldValue::new(
                    "observed",
                    StructuredInfoValue::variant(observed_type.clone(), disposition, unit).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let allocations = allocation::allocations(|| {
            let bytes = evaluator.evaluate(&observation).unwrap();
            let value = validate_canonical_structured_value(bytes).unwrap();
            assert!(
                value
                    .record_field("observed")
                    .unwrap()
                    .unwrap()
                    .variant_payload(disposition)
                    .unwrap()
                    .is_some()
            );
        });
        assert_eq!(allocations, 0);
    }
}

#[test]
fn two_capture_window_reuses_slots_across_128_observations_with_constant_state_bound() {
    let [initialize, insert, drain] = programs([
        "usb-hid-mouse-order-initialize",
        "usb-hid-mouse-order-insert",
        "usb-hid-mouse-order-drain",
    ]);
    let mut state = initialize.evaluate(&[]).unwrap();
    let mut full_state_size = None;
    for round in 0..64_u64 {
        let first = round * 2;
        for ordinal in [first + 1, first] {
            state = payload(
                &insert
                    .evaluate(&insertion(&insert.input_type, &state, ordinal, "short"))
                    .unwrap(),
                "accepted",
            );
            assert!(state.len() <= 4096);
            if ordinal == first {
                match full_state_size {
                    Some(size) => assert_eq!(state.len(), size),
                    None => full_state_size = Some(state.len()),
                }
            }
            if ordinal == first + 1 {
                assert_tag(&drain.evaluate(&state).unwrap(), "waiting");
            }
        }
        for ordinal in first..first + 2 {
            let result = drain.evaluate(&state).unwrap();
            let ready = validate_canonical_structured_value(&result)
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
            state = encoded(ready.record_field("state").unwrap().unwrap());
        }
        assert_tag(&drain.evaluate(&state).unwrap(), "waiting");
    }
}

#[path = "usb_mouse_order/pointer_history.rs"]
mod pointer_history;

#[path = "usb_mouse_order/endpoint_decode.rs"]
mod endpoint_decode;
