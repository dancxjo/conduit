//! Explicit finite repetition preserves every Source-authored packet deadline.
use super::*;
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};

#[test]
fn nine_repeat_request_keeps_all_seventy_two_frames_and_pcm_packets_exact() {
    let text = source().replace(
        include_str!("../../plots/usb/audio-playback-policy.conduit"),
        include_str!("../../plots/usb/audio-repeat-playback-policy.conduit"),
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&text), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "usb-audio-playback-request",
        &ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let request = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = request.input_type.shape() else {
        panic!("state")
    };
    let pcm_type = fields
        .iter()
        .find(|field| field.name() == "pcm")
        .unwrap()
        .value_type();
    let pcm = block_value(pcm_type, "", 0);
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&request).unwrap();
    for first in [0u64, 2045] {
        let state = StructuredInfoValue::record(
            request.input_type.clone(),
            fields
                .iter()
                .map(|field| {
                    let value = match field.name() {
                        "pcm" => pcm.clone(),
                        "first-frame" => StructuredInfoValue::leaf(
                            field.value_type().clone(),
                            first.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                        "phase" => StructuredInfoValue::leaf(
                            field.value_type().clone(),
                            1u64.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                        "terminal" => {
                            StructuredInfoValue::leaf(field.value_type().clone(), vec![0]).unwrap()
                        }
                        _ => panic!("state field"),
                    };
                    StructuredFieldValue::new(field.name(), value).unwrap()
                })
                .collect(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let output = evaluator.evaluate(&state).unwrap();
        let value = validate_canonical_structured_value(output).unwrap();
        let packets = value
            .variant_payload("batch")
            .unwrap()
            .unwrap()
            .record_field("packets")
            .unwrap()
            .unwrap();
        assert_eq!(packets.collection_length().unwrap(), 72);
        let original = pcm.canonical_bytes().unwrap();
        let originals = validate_canonical_structured_value(&original)
            .unwrap()
            .record_field("packets")
            .unwrap()
            .unwrap();
        for index in 0..72u16 {
            let packet = packets.collection_index(index).unwrap().unwrap();
            let frame = packet
                .record_field("frame")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap();
            assert_eq!(
                frame,
                &((first + u64::from(index)) % 2048).to_le_bytes(),
                "packet {index}"
            );
            assert_eq!(
                packet
                    .record_field("wire")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bytes")
                    .unwrap(),
                originals
                    .collection_index(index % 8)
                    .unwrap()
                    .unwrap()
                    .record_field("wire")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bytes")
                    .unwrap()
            );
        }
    }
}

// Append inside usb_audio_plots/repeat.rs. Hosted Source regression only;
// canonical completions below are synthetic, never native event evidence.

fn repeat_completion_program(
    checked: &conduit_plot::CheckedSyntaxDocument,
    entry: &str,
) -> PortableExpressionProgram {
    let expanded = expand_canonical_plot_for_authoring(checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1, "{entry}");
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("{entry}: prepared expression")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

fn repeat_completion_observation(
    ty: &conduit_core::StructuredInfoType,
    change: Option<(usize, &str)>,
    tag: &str,
) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("observation")
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                let value = match field.name() {
                    "result" => {
                        super::completion::input(field.value_type(), "/result", 0, change, 72, tag)
                    }
                    "previous" => {
                        let StructuredInfoTypeShape::Record { fields, .. } =
                            field.value_type().shape()
                        else {
                            panic!("previous state")
                        };
                        StructuredInfoValue::record(
                            field.value_type().clone(),
                            fields
                                .iter()
                                .map(|state_field| {
                                    let value = match state_field.name() {
                                        "pcm" => block_value(state_field.value_type(), "", 0),
                                        "first-frame" => StructuredInfoValue::leaf(
                                            state_field.value_type().clone(),
                                            2045u64.to_le_bytes().to_vec(),
                                        )
                                        .unwrap(),
                                        "phase" => StructuredInfoValue::leaf(
                                            state_field.value_type().clone(),
                                            1u64.to_le_bytes().to_vec(),
                                        )
                                        .unwrap(),
                                        "terminal" => StructuredInfoValue::leaf(
                                            state_field.value_type().clone(),
                                            vec![0],
                                        )
                                        .unwrap(),
                                        other => panic!("unexpected state field {other}"),
                                    };
                                    StructuredFieldValue::new(state_field.name(), value).unwrap()
                                })
                                .collect(),
                        )
                        .unwrap()
                    }
                    other => panic!("unexpected observation field {other}"),
                };
                StructuredFieldValue::new(field.name(), value).unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

fn repeat_completion_flag(bytes: &[u8], field: &str) -> bool {
    let value = validate_canonical_structured_value(bytes).unwrap();
    let flag = value
        .record_field(field)
        .unwrap()
        .unwrap()
        .primitive_bytes("value/bool")
        .unwrap();
    assert!(matches!(flag, [0] | [1]));
    flag == [1]
}

#[test]
fn all_nine_completion_stages_require_each_original_frame_requested_and_actual_extent() {
    // Compile actual repository repeat policy once; each of the nine prepared
    // validation programs is prepared once and reused for all 216 mutations.
    let text = source().replace(
        include_str!("../../plots/usb/audio-playback-policy.conduit"),
        include_str!("../../plots/usb/audio-repeat-playback-policy.conduit"),
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&text), &StartupCatalog::new()).unwrap();
    let seed_program = repeat_completion_program(&checked, "usb-audio-completion-validation-seed");
    let mut seed = PreparedPortableExpressionEvaluator::new(&seed_program).unwrap();
    let stage_programs: Vec<_> = (0..9)
        .map(|stage| {
            repeat_completion_program(
                &checked,
                &format!("usb-audio-completion-validation-{stage}"),
            )
        })
        .collect();
    let mut stages: Vec<_> = stage_programs
        .iter()
        .map(|p| PreparedPortableExpressionEvaluator::new(p).unwrap())
        .collect();
    let finish_program =
        repeat_completion_program(&checked, "usb-audio-completion-validation-finish");
    let mut finish = PreparedPortableExpressionEvaluator::new(&finish_program).unwrap();

    for change in core::iter::once(None).chain(
        (0..72)
            .flat_map(|index| ["frame", "requested", "actual"].map(|field| Some((index, field)))),
    ) {
        // The shared completion fixture encodes frames (2045+index)%2048,
        // requested=actual=192. This covers wrap at index3 and the prior bug32.
        let observation =
            repeat_completion_observation(&seed_program.input_type, change, "completed");
        let mut value = seed.evaluate(&observation).unwrap().to_vec();
        assert!(repeat_completion_flag(&value, "accepted"));
        for (stage, evaluator) in stages.iter_mut().enumerate() {
            value = evaluator.evaluate(&value).unwrap().to_vec();
            let expected = change.is_none_or(|(index, _)| stage < index / 8);
            assert_eq!(
                repeat_completion_flag(&value, "accepted"),
                expected,
                "stage {stage}, changed packet/field {change:?}"
            );
        }
        let finished = finish.evaluate(&value).unwrap();
        let state = validate_canonical_structured_value(finished).unwrap();
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
            repeat_completion_flag(finished, "terminal"),
            change.is_some()
        );
        assert_eq!(
            state
                .record_field("first-frame")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            2045u64.to_le_bytes()
        );
    }
}

#[test]
fn public_fault_observation_preserves_original_tag_and_prefix_before_terminal_stop() {
    let text = source().replace(
        include_str!("../../plots/usb/audio-playback-policy.conduit"),
        include_str!("../../plots/usb/audio-repeat-playback-policy.conduit"),
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&text), &StartupCatalog::new()).unwrap();
    let projection_program = repeat_completion_program(&checked, "usb-audio-playback-result");
    // Public Fore contract must exactly match the selected expression Back.
    assert_eq!(projection_program.maximum_prepared_output_bytes().unwrap(), 7864);
    let mut projection = PreparedPortableExpressionEvaluator::new(&projection_program).unwrap();
    let seed_program = repeat_completion_program(&checked, "usb-audio-completion-validation-seed");
    let mut seed = PreparedPortableExpressionEvaluator::new(&seed_program).unwrap();
    let programs: Vec<_> = (0..9)
        .map(|stage| {
            repeat_completion_program(
                &checked,
                &format!("usb-audio-completion-validation-{stage}"),
            )
        })
        .collect();
    let mut stages: Vec<_> = programs
        .iter()
        .map(|p| PreparedPortableExpressionEvaluator::new(p).unwrap())
        .collect();
    let finish_program =
        repeat_completion_program(&checked, "usb-audio-completion-validation-finish");
    let mut finish = PreparedPortableExpressionEvaluator::new(&finish_program).unwrap();
    for tag in ["short", "missed-service", "underrun", "provider-lost"] {
        // Synthetic canonical transport observations; this is Source policy proof,
        // not evidence of hardware or native controller fault delivery.
        let observation = repeat_completion_observation(&seed_program.input_type, None, tag);
        let original = validate_canonical_structured_value(&observation).unwrap();
        let expected = original.record_field("result").unwrap().unwrap();
        let projected = projection.evaluate(&observation).unwrap();
        let result = validate_canonical_structured_value(projected).unwrap();
        assert_eq!(result, expected, "{tag}: exact type and all payload bytes");
        assert!(result.variant_payload(tag).unwrap().is_some(), "{tag}");
        let mut validation = seed.evaluate(&observation).unwrap().to_vec();
        assert!(!repeat_completion_flag(&validation, "accepted"), "{tag}");
        for stage in &mut stages {
            validation = stage.evaluate(&validation).unwrap().to_vec();
            assert!(!repeat_completion_flag(&validation, "accepted"), "{tag}");
        }
        let state = finish.evaluate(&validation).unwrap();
        assert!(repeat_completion_flag(state, "terminal"), "{tag}");
        let value = validate_canonical_structured_value(state).unwrap();
        assert_eq!(
            value
                .record_field("phase")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            &1u64.to_le_bytes()
        );
    }
}
