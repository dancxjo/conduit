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
    let routing = program("usb-audio-playback-route");
    let mut router = PreparedPortableExpressionEvaluator::new(&routing).unwrap();
    assert!(
        validate_canonical_structured_value(router.evaluate(&initial).unwrap())
            .unwrap()
            .variant_payload("awaiting")
            .unwrap()
            .is_some()
    );
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

fn clock_input(
    ty: &conduit_core::StructuredInfoType,
    path: &str,
    changed: &str,
) -> StructuredInfoValue {
    use conduit_core::StructuredInfoTypeShape as Shape;
    match ty.shape() {
        Shape::Record { .. } if path.ends_with("pcm") => block_value(ty, "", 0),
        Shape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        clock_input(
                            field.value_type(),
                            &format!("{path}/{}", field.name()),
                            changed,
                        ),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        Shape::Variant { cases, .. } => {
            let tag = if changed == "not-snapshot" {
                "drained"
            } else {
                "snapshot"
            };
            let case = cases.iter().find(|case| case.tag() == tag).unwrap();
            StructuredInfoValue::variant(
                ty.clone(),
                tag,
                clock_input(case.payload_type(), &format!("{path}/{tag}"), changed),
            )
            .unwrap()
        }
        Shape::Leaf(kind) => {
            let field = path.rsplit('/').next().unwrap();
            let value: u64 = match field {
                "current-microframe" => 16376,
                "scheduling-threshold" => 248,
                "service-interval-microframes" | "maximum-pending-packets" => 8,
                "maximum-packet-bytes" => 192,
                "dma-resource-bytes" => 3072,
                _ => 0,
            };
            let value = if field == changed { value + 1 } else { value };
            let bytes = if kind.as_str() == "value/empty" {
                vec![]
            } else if kind.as_str() == "value/bool" {
                vec![value as u8]
            } else {
                value.to_le_bytes().to_vec()
            };
            StructuredInfoValue::leaf(ty.clone(), bytes).unwrap()
        }
        _ => panic!("unsupported clock fixture"),
    }
}

#[test]
fn only_a_supported_snapshot_advances_playback() {
    let p = program("usb-audio-playback-clock-advance");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for change in [
        "",
        "not-snapshot",
        "phase",
        "terminal",
        "scheduling-threshold",
        "service-interval-microframes",
        "maximum-packet-bytes",
        "pending-packets",
        "maximum-pending-packets",
        "dma-resource-bytes",
    ] {
        let input = clock_input(&p.input_type, "", change)
            .canonical_bytes()
            .unwrap();
        let output = evaluator.evaluate(&input).unwrap();
        let state = validate_canonical_structured_value(output).unwrap();
        assert_eq!(
            state
                .record_field("terminal")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            [u8::from(!change.is_empty())],
            "{change}"
        );
        if change.is_empty() {
            assert_eq!(
                state
                    .record_field("phase")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                1u64.to_le_bytes()
            );
            assert_eq!(
                state
                    .record_field("first-frame")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                31u64.to_le_bytes()
            );
        }
    }
}

#[test]
fn batch_requires_all_original_deadlines_and_extents_before_drain() {
    let p = program("usb-audio-playback-batch-advance");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for change in [
        None,
        Some((0, "frame")),
        Some((7, "requested")),
        Some((4, "actual")),
    ] {
        let StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
            panic!("observation")
        };
        let input = StructuredInfoValue::record(
            p.input_type.clone(),
            fields
                .iter()
                .map(|field| {
                    let value = if field.name() == "result" {
                        super::completion::input(
                            field.value_type(),
                            "/result",
                            0,
                            change,
                            8,
                            "completed",
                        )
                    } else {
                        let StructuredInfoTypeShape::Record { fields, .. } =
                            field.value_type().shape()
                        else {
                            panic!("state")
                        };
                        StructuredInfoValue::record(
                            field.value_type().clone(),
                            fields
                                .iter()
                                .map(|f| {
                                    let value = if f.name() == "pcm" {
                                        block_value(f.value_type(), "", 0)
                                    } else {
                                        let bytes = match f.name() {
                                            "first-frame" => 2045u64.to_le_bytes().to_vec(),
                                            "phase" => 1u64.to_le_bytes().to_vec(),
                                            "terminal" => vec![0],
                                            _ => panic!("field"),
                                        };
                                        StructuredInfoValue::leaf(f.value_type().clone(), bytes)
                                            .unwrap()
                                    };
                                    StructuredFieldValue::new(f.name(), value).unwrap()
                                })
                                .collect(),
                        )
                        .unwrap()
                    };
                    StructuredFieldValue::new(field.name(), value).unwrap()
                })
                .collect(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let state =
            validate_canonical_structured_value(evaluator.evaluate(&input).unwrap()).unwrap();
        assert_eq!(
            state
                .record_field("phase")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            (if change.is_none() { 2u64 } else { 1u64 }).to_le_bytes()
        );
        assert_eq!(
            state
                .record_field("terminal")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            [u8::from(change.is_some())]
        );
    }
}

#[test]
fn one_isochronous_offer_prepares_the_typed_playback_loop() {
    use conduitos::protocol_source::{
        PreparedProtocolSource, ProtocolSourcePackage, ProtocolSpecializationRequest as Request,
        ProtocolValueReference,
    };
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    let text = format!(
        "{}\n{}",
        source(),
        include_str!("../../plots/usb/audio-playback-back.conduit")
    );
    let package = ProtocolSourcePackage::compile(
        text,
        &[
            Request::FeedbackZipInstance {
                alias: "UsbAudioInitializationJoin".into(),
                paired_alias: "UsbAudioInitializationPair".into(),
                left: value("UsbAudioPlaybackState"),
                right: value("UsbAudioAdmission"),
            },
            Request::Merge {
                value: value("UsbAudioPlaybackState"),
            },
            Request::SeededUntilInstance {
                alias: "UsbAudioPlaybackCell".into(),
                value: value("UsbAudioPlaybackState"),
            },
            Request::FeedbackZipInstance {
                alias: "UsbAudioPlaybackJoin".into(),
                paired_alias: "UsbAudioPlaybackPair".into(),
                left: value("UsbAudioPlaybackState"),
                right: value("UsbIsochronousOutResult"),
            },
        ],
    )
    .unwrap();
    let prepared = PreparedProtocolSource::prepare(package).unwrap();
    for entry in [
        "usb-audio-playback-seed",
        "usb-audio-playback-route",
        "usb-audio-playback-request",
        "usb-audio-playback-observation",
        "usb-audio-playback-clock-advance",
        "usb-audio-playback-batch-advance",
        "usb-audio-playback-drain-advance",
        "usb-audio-playback-observation-route",
    ] {
        prepared
            .expand(entry)
            .unwrap_or_else(|error| panic!("{entry}: {error:?}"));
    }
    let expanded = prepared.expand("usb-audio-playback-admitted").unwrap();
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "machine/usb/isochronous-out")
            .count(),
        1
    );
}
