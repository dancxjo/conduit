//! Prepared Source projections preserve PCM and explicit controller deadlines.
use super::*;
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};

#[test]
fn playback_observes_before_submitting_exact_pcm_and_wraps_frames() {
    let seed = program("usb-audio-playback-seed");
    let pcm = block_value(&seed.input_type, "", 0);
    let original = pcm.canonical_bytes().unwrap();
    let mut seed_evaluator = PreparedPortableExpressionEvaluator::new(&seed).unwrap();
    let initial = seed_evaluator.evaluate(&original).unwrap().to_vec();
    assert!(initial.len() <= 4096);
    let request = program("usb-audio-playback-request");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&request).unwrap();
    let observed = evaluator.evaluate(&initial).unwrap();
    assert!(validate_canonical_structured_value(observed)
        .unwrap()
        .variant_payload("observe")
        .unwrap()
        .is_some());
    let StructuredInfoTypeShape::Record { fields, .. } = request.input_type.shape() else {
        panic!("state record")
    };
    for phase in [1u64, 2] {
        let state = StructuredInfoValue::record(
            request.input_type.clone(),
            fields
                .iter()
                .map(|field| {
                    let value = match field.name() {
                        "pcm" => pcm.clone(),
                        "first-frame" => StructuredInfoValue::leaf(
                            field.value_type().clone(),
                            2045u64.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                        "phase" => StructuredInfoValue::leaf(
                            field.value_type().clone(),
                            phase.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                        "terminal" => {
                            StructuredInfoValue::leaf(field.value_type().clone(), vec![0]).unwrap()
                        }
                        _ => panic!("unexpected state field"),
                    };
                    StructuredFieldValue::new(field.name(), value).unwrap()
                })
                .collect(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let output = evaluator.evaluate(&state).unwrap();
        let submitted = validate_canonical_structured_value(output).unwrap();
        if phase == 2 {
            assert!(submitted.variant_payload("drain").unwrap().is_some());
            continue;
        }
        let packets = submitted
            .variant_payload("batch")
            .unwrap()
            .unwrap()
            .record_field("packets")
            .unwrap()
            .unwrap();
        let original_value = validate_canonical_structured_value(&original).unwrap();
        let originals = original_value.record_field("packets").unwrap().unwrap();
        assert_eq!(packets.collection_length().unwrap(), 8);
        for index in 0..8 {
            let packet = packets.collection_index(index).unwrap().unwrap();
            let expected = originals.collection_index(index).unwrap().unwrap();
            assert_eq!(
                packet
                    .record_field("wire")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bytes")
                    .unwrap(),
                expected
                    .record_field("wire")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bytes")
                    .unwrap()
            );
            assert_eq!(
                packet
                    .record_field("frame")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                ((2045 + index as u64) % 2048).to_le_bytes()
            );
        }
    }
}
