#[path = "../../../architecture/plot/tests/prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{ConfigurationValue, validate_canonical_structured_value};
use conduit_plot::{
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};
fn source() -> String {
    let audio = include_str!("../../../semantics/audio/types.conduit");
    let start = audio.find("type PcmSampleRepresentation").unwrap();
    let end = audio.find("type AudioRenderDemand").unwrap();
    let negotiation = include_str!("../plots/usb/audio-negotiation-back.conduit");
    let setup_types = &negotiation[negotiation.find("type UsbAudioSetupState").unwrap()
        ..negotiation.find("plot usb-audio-setup-start").unwrap()];
    let setup_observation = &negotiation[negotiation
        .find("plot usb-audio-setup-observation")
        .unwrap()
        ..negotiation.find("plot usb-audio-setup-accept").unwrap()];
    let usb = include_str!("../plots/usb/protocol.conduit");
    let cadence_start = usb.find("type UsbPcmCadence").unwrap();
    let cadence_end = usb.find("# A stale DMA completion").unwrap();
    [
        &audio[start..end],
        setup_types,
        setup_observation,
        &usb[cadence_start..cadence_end],
        include_str!("../plots/usb/control-types.conduit"),
        include_str!("../plots/usb/isochronous-out-types.conduit"),
        include_str!("../plots/usb/audio-types.conduit"),
        include_str!("../plots/usb/audio-block-types.conduit"),
        include_str!("../plots/usb/audio-block-profile.conduit"),
        include_str!("../plots/usb/audio-profile.conduit"),
    ]
    .join("\n")
}
fn program(entry: &str) -> PortableExpressionProgram {
    let text = source();
    program_from_text(&text, entry)
}
fn program_from_text(text: &str, entry: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(text);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("pure checked program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
#[test]
fn every_audio_policy_plot_checks_and_expands() {
    let text = source();
    let syntax = parse_syntax_document(&text);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    for plot in &checked.plots {
        expand_canonical_plot_for_authoring(&checked, &plot.name, &ProfileCatalog::new())
            .unwrap_or_else(|error| panic!("{}: {error:?}", plot.name));
    }
}
#[test]
fn class_control_words_are_exact_usb_wire_not_native_policy() {
    for (entry, expected, count) in [
        (
            "usb-audio-configuration-request",
            [0, 9, 1, 0, 0, 0, 0, 0],
            0,
        ),
        ("usb-audio-alternate-request", [1, 11, 1, 0, 1, 0, 0, 0], 0),
        ("usb-audio-set-rate-request", [0x22, 1, 0, 1, 1, 0, 3, 0], 3),
        (
            "usb-audio-get-rate-request",
            [0xa2, 0x81, 0, 1, 1, 0, 3, 0],
            0,
        ),
    ] {
        let p = program(entry);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let value = validate_canonical_structured_value(evaluator.evaluate(&[1]).unwrap()).unwrap();
        assert_eq!(
            value
                .record_field("setup")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            expected
        );
        assert_eq!(
            value
                .record_field("output")
                .unwrap()
                .unwrap()
                .collection_length()
                .unwrap(),
            count
        );
    }
}

fn completed(
    program: &PortableExpressionProgram,
    bytes: &[u8],
    transferred: u16,
    short: bool,
) -> Vec<u8> {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoTypeShape as Shape, StructuredInfoValue as Value,
    };
    let Shape::Variant { cases, .. } = program.input_type.shape() else {
        panic!("completion variant")
    };
    let payload = cases
        .iter()
        .find(|case| case.tag() == "completed")
        .unwrap()
        .payload_type();
    let Shape::Record { fields, .. } = payload.shape() else {
        panic!("completed record")
    };
    let values = fields
        .iter()
        .map(|field| {
            let value = match field.name() {
                "transferred" => Value::leaf(
                    field.value_type().clone(),
                    transferred.to_le_bytes().to_vec(),
                )
                .unwrap(),
                "short" => Value::leaf(field.value_type().clone(), vec![u8::from(short)]).unwrap(),
                "input" => {
                    let Shape::Sequence { element, .. } = field.value_type().shape() else {
                        panic!("bytes sequence")
                    };
                    Value::sequence(
                        field.value_type().clone(),
                        bytes
                            .iter()
                            .map(|byte| Value::leaf(element.clone(), vec![*byte]).unwrap())
                            .collect(),
                    )
                    .unwrap()
                }
                _ => panic!("unknown completion field"),
            };
            StructuredFieldValue::new(field.name(), value).unwrap()
        })
        .collect();
    let payload = Value::record(payload.clone(), values).unwrap();
    Value::variant(program.input_type.clone(), "completed", payload)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
#[test]
fn class_rate_negotiation_refuses_rounding_short_and_extent_lies() {
    let p = program("usb-audio-get-rate-ack");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for (bytes, transferred, short, accepted) in [
        (&[128, 187, 0][..], 3, false, true),
        (&[68, 172, 0][..], 3, false, false),
        (&[128, 187][..], 2, true, false),
        (&[128, 187, 0][..], 3, true, false),
        (&[128, 187, 0][..], 2, false, false),
    ] {
        assert_eq!(
            evaluator
                .evaluate(&completed(&p, bytes, transferred, short))
                .unwrap(),
            [u8::from(accepted)]
        );
    }
}
#[test]
fn descriptor_policy_rejects_changed_format_rate_alt_and_endpoint() {
    // Exact external device fixture descriptor; this is a synthetic policy test.
    let descriptor = bytes_from_hex(include_str!("../plots/usb/proof/audio/configuration.hex"));
    let p = program("usb-audio-profile-supported");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(
        evaluator
            .evaluate(&completed(&p, &descriptor, 100, false))
            .unwrap(),
        [1]
    );
    for index in [4, 5, 12, 59, 62, 77, 78, 79, 81, 86, 87, 88, 90, 96] {
        let mut changed = descriptor.clone();
        changed[index] ^= 1;
        assert_eq!(
            evaluator
                .evaluate(&completed(&p, &changed, 100, false))
                .unwrap(),
            [0],
            "descriptor byte{index}"
        );
    }
    assert_eq!(
        evaluator
            .evaluate(&completed(&p, &descriptor[..99], 99, true))
            .unwrap(),
        [0]
    );
}
fn bytes_from_hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

#[test]
fn repeated_checked_rate_observation_allocates_nothing() {
    let p = program("usb-audio-get-rate-ack");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let input = completed(&p, &[128, 187, 0], 3, false);
    assert_eq!(
        allocation::allocations(|| {
            for _ in 0..1024 {
                assert_eq!(evaluator.evaluate(&input).unwrap(), [1]);
            }
        }),
        0
    );
}

fn block_value(
    ty: &conduit_core::StructuredInfoType,
    path: &str,
    packet: usize,
) -> conduit_core::StructuredInfoValue {
    block_value_with(ty, path, packet, None)
}
fn block_value_with(
    ty: &conduit_core::StructuredInfoType,
    path: &str,
    packet: usize,
    replacement: Option<(&str, u64)>,
) -> conduit_core::StructuredInfoValue {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoTypeShape as Shape, StructuredInfoValue as V,
    };
    match ty.shape() {
        Shape::Record { fields, .. } => V::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    let next = format!("{path}/{}", field.name());
                    StructuredFieldValue::new(
                        field.name(),
                        block_value_with(field.value_type(), &next, packet, replacement),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        Shape::Variant { cases, .. } => {
            let tag = if path.ends_with("representation") {
                "signed16-little-endian"
            } else {
                "stereo-left-right"
            };
            let case = cases.iter().find(|case| case.tag() == tag).unwrap();
            V::variant(
                ty.clone(),
                tag,
                V::leaf(case.payload_type().clone(), vec![]).unwrap(),
            )
            .unwrap()
        }
        Shape::Sequence { element, .. } => V::sequence(
            ty.clone(),
            (0..8)
                .map(|index| block_value_with(element, path, index, replacement))
                .collect(),
        )
        .unwrap(),
        Shape::Leaf(kind) => {
            let value: u64 = match path.rsplit('/').next().unwrap() {
                "sample-rate-hz" | "sample_rate" => 48000,
                "frame-count" => 384,
                "clock-id" => 1,
                "payload-bytes" => 1536,
                "channels" | "sample_bytes" => 2,
                "frame" => 100 + packet as u64,
                "start-frame" | "discontinuity" | "phase" | "wire" => 0,
                other => panic!("unknown fixture field {other}"),
            };
            let value = replacement
                .filter(|(target, _)| *target == path)
                .map_or(value, |(_, replacement)| replacement);
            let width = match kind.as_str() {
                "value/u64" => 8,
                "value/u32" => 4,
                "value/u16" => 2,
                "value/u8" | "value/bool" => 1,
                "value/bytes" => 0,
                other => panic!("kind {other}"),
            };
            let bytes = if width == 0 {
                (0..replacement
                    .filter(|(target, _)| *target == path)
                    .map_or(192, |(_, length)| length as usize))
                    .map(|offset| ((packet * 192 + offset) % 251) as u8)
                    .collect()
            } else {
                value.to_le_bytes()[..width].to_vec()
            };
            V::leaf(ty.clone(), bytes).unwrap()
        }
        _ => panic!("unsupported block shape"),
    }
}

#[test]
fn whole_pcm_block_has_measured_canonical_envelope_and_exact_transport_projection() {
    let p = program("usb-audio-block-header-supported");
    let bytes = block_value(&p.input_type, "", 0).canonical_bytes().unwrap();
    println!("whole PCM block canonical bytes: {}", bytes.len());
    assert!(bytes.len() <= 4096);
    validate_canonical_structured_value(&bytes).unwrap();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(evaluator.evaluate(&bytes).unwrap(), [1]);
    let request = program("usb-audio-block-request");
    let mut encoder = PreparedPortableExpressionEvaluator::new(&request).unwrap();
    let output = encoder.evaluate(&bytes).unwrap();
    let original = validate_canonical_structured_value(&bytes).unwrap();
    let submitted = validate_canonical_structured_value(output).unwrap();
    let packets = original.record_field("packets").unwrap().unwrap();
    let transported = submitted
        .variant_payload("batch")
        .unwrap()
        .unwrap()
        .record_field("packets")
        .unwrap()
        .unwrap();
    assert_eq!(packets.collection_length().unwrap(), 8);
    assert_eq!(transported.collection_length().unwrap(), 8);
    for index in 0..8 {
        let a = packets.collection_index(index).unwrap().unwrap();
        let b = transported.collection_index(index).unwrap().unwrap();
        for (field, kind) in [("frame", "value/u64"), ("wire", "value/bytes")] {
            assert_eq!(
                a.record_field(field)
                    .unwrap()
                    .unwrap()
                    .primitive_bytes(kind)
                    .unwrap(),
                b.record_field(field)
                    .unwrap()
                    .unwrap()
                    .primitive_bytes(kind)
                    .unwrap()
            );
        }
    }
}

#[test]
fn whole_pcm_block_packet_and_adjacent_laws_prepare_and_execute() {
    let header = program("usb-audio-block-header-supported");
    let bytes = block_value(&header.input_type, "", 0)
        .canonical_bytes()
        .unwrap();
    for index in 0..8 {
        let p = program(&format!("usb-audio-block-packet-{index}-supported"));
        assert_eq!(
            PreparedPortableExpressionEvaluator::new(&p)
                .unwrap()
                .evaluate(&bytes)
                .unwrap(),
            [1]
        );
    }
    for index in 0..7 {
        let p = program(&format!("usb-audio-block-adjacent-{index}-supported"));
        assert_eq!(
            PreparedPortableExpressionEvaluator::new(&p)
                .unwrap()
                .evaluate(&bytes)
                .unwrap(),
            [1]
        );
    }
}

#[test]
fn whole_pcm_header_refuses_clock_phase_discontinuity_and_extent_changes() {
    let p = program("usb-audio-block-header-supported");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for (field, value) in [
        ("/header/sample-rate-hz", 44100),
        ("/header/frame-count", 383),
        ("/header/clock-id", 0),
        ("/header/payload-bytes", 1535),
        ("/header/discontinuity", 1),
        ("/header/start-frame", u64::MAX),
        ("/cadence/sample_rate", 44100),
        ("/cadence/phase", 1),
        ("/cadence/channels", 1),
        ("/cadence/sample_bytes", 1),
    ] {
        let encoded = block_value_with(&p.input_type, "", 0, Some((field, value)))
            .canonical_bytes()
            .unwrap();
        if let Ok(output) = evaluator.evaluate(&encoded) {
            assert_eq!(output, [0], "{field}");
        }
    }
}

#[test]
fn setup_observation_preserves_each_exact_phase_ack_without_pairing_raw_result() {
    let p = program("usb-audio-setup-observation");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let descriptor = bytes_from_hex(include_str!("../plots/usb/proof/audio/configuration.hex"));
    for (bytes, count, short, expected) in [
        (descriptor.as_slice(), 100, false, [1, 0, 0, 0]),
        (&[][..], 0, false, [0, 1, 0, 0]),
        (&[][..], 3, false, [0, 0, 1, 0]),
        (&[128, 187, 0][..], 3, false, [0, 0, 0, 1]),
        (&[68, 172, 0][..], 3, false, [0, 0, 0, 0]),
        (&[128, 187, 0][..], 3, true, [0, 0, 0, 0]),
    ] {
        let input = completed(&p, bytes, count, short);
        let output =
            validate_canonical_structured_value(evaluator.evaluate(&input).unwrap()).unwrap();
        for (field, expected) in ["descriptor", "zero-ack", "set-ack", "get-ack"]
            .into_iter()
            .zip(expected)
        {
            assert_eq!(
                output
                    .record_field(field)
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bool")
                    .unwrap(),
                [expected],
                "{field}"
            );
        }
    }
}

#[test]
fn ordinary_protocol_source_prepares_setup_once_with_exact_small_feedback_event() {
    use conduitos::protocol_source::{
        PreparedProtocolSource, ProtocolSourcePackage, ProtocolSpecializationRequest,
        ProtocolValueReference,
    };
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    let package = ProtocolSourcePackage::compile(
        include_str!("../plots/usb/audio-negotiation-back.conduit").into(),
        &[
            ProtocolSpecializationRequest::Singleton {
                value: value("UsbAudioSetupState"),
            },
            ProtocolSpecializationRequest::SeededUntil {
                value: value("UsbAudioSetupState"),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: value("UsbAudioSetupState"),
                right: value("UsbAudioSetupEvent"),
            },
        ],
    )
    .unwrap();
    let source = PreparedProtocolSource::prepare(package).unwrap();
    let expanded = source.expand("usb-audio-negotiate").unwrap();
    assert!(
        expanded
            .expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == "machine/usb/control")
    );
}

fn playback_package() -> conduitos::protocol_source::ProtocolSourcePackage {
    use conduitos::protocol_source::{
        ProtocolSourcePackage, ProtocolSpecializationRequest as S, ProtocolValueReference,
    };
    let audio = include_str!("../../../semantics/audio/types.conduit");
    let pcm = &audio[audio.find("type PcmSampleRepresentation").unwrap()
        ..audio.find("type AudioRenderDemand").unwrap()];
    let usb = include_str!("../plots/usb/protocol.conduit");
    let cadence = &usb
        [usb.find("type UsbPcmCadence").unwrap()..usb.find("# A stale DMA completion").unwrap()];
    let reference = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    ProtocolSourcePackage::compile(
        [
            include_str!("../plots/usb/audio-playback-back.conduit"),
            pcm,
            cadence,
            include_str!("../plots/usb/audio-playback-types.conduit"),
            include_str!("../plots/usb/audio-playback-policy.conduit"),
        ]
        .join("\n"),
        &[
            S::SeededUntil {
                value: reference("UsbAudioPlaybackState"),
            },
            S::FeedbackZip {
                left: reference("UsbAudioPlaybackState"),
                right: reference("UsbAudioPlaybackEvent"),
            },
            S::Merge {
                value: reference("UsbAudioPlaybackEvent"),
            },
        ],
    )
    .unwrap()
}
#[test]
fn checked_playback_source_prepares_control_clock_batch_and_explicit_drain() {
    use conduitos::protocol_source::PreparedProtocolSource;
    let source = PreparedProtocolSource::prepare(playback_package()).unwrap();
    let expanded = source.expand("usb-audio-playback").unwrap();
    for expected in ["machine/usb/control", "machine/usb/isochronous-out"] {
        assert!(
            expanded
                .expanded
                .gears
                .iter()
                .any(|gear| gear.kind_id.as_str() == expected)
        );
    }
}

#[test]
fn playback_authored_pcm_canonical_envelope_measures_lawful_and_declared_maximum() {
    let text = [
        source(),
        include_str!("../plots/usb/audio-playback-types.conduit").into(),
        include_str!("../plots/usb/audio-playback-policy.conduit").into(),
    ]
    .join("\n");
    let p = program_from_text(&text, "usb-audio-authored-pcm-supported");
    let lawful = block_value(&p.input_type, "", 0).canonical_bytes().unwrap();
    let maximum = block_value_with(&p.input_type, "", 0, Some(("/segments/wire", 256)))
        .canonical_bytes()
        .unwrap();
    println!(
        "authored PCM canonical lawful={} declared-eight-by-256={}",
        lawful.len(),
        maximum.len()
    );
    assert_eq!(maximum.len() - lawful.len(), 8 * (256 - 192));
    assert!(
        maximum.len() <= 4096,
        "authored input needs a reviewed separate envelope"
    );
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(evaluator.evaluate(&lawful).unwrap(), [1]);
    assert_eq!(evaluator.evaluate(&maximum).unwrap(), [0]);
    // This measures the original author input; it does not assert that the
    // separately prepared feedback pair or native retained resources fit it.
}

fn prepared_playback_program(entry: &str) -> PortableExpressionProgram {
    let source =
        conduitos::protocol_source::PreparedProtocolSource::prepare(playback_package()).unwrap();
    let expanded = source.expand(entry).unwrap().expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("pure checked playback program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

fn playback_envelope_value(
    ty: &conduit_core::StructuredInfoType,
    path: &str,
    wire_length: usize,
    phase: u64,
    event_kind: u64,
    event_status: &str,
) -> conduit_core::StructuredInfoValue {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoTypeShape as S, StructuredInfoValue as V,
    };
    match ty.shape() {
        S::Record { fields, .. } => V::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    let next = format!("{path}/{}", field.name());
                    let value = if field.name() == "header" {
                        block_value(field.value_type(), &next, 0)
                    } else {
                        playback_envelope_value(
                            field.value_type(),
                            &next,
                            wire_length,
                            phase,
                            event_kind,
                            event_status,
                        )
                    };
                    StructuredFieldValue::new(field.name(), value).unwrap()
                })
                .collect(),
        )
        .unwrap(),
        S::Sequence {
            element,
            maximum_items,
            ..
        } => V::sequence(
            ty.clone(),
            (0..maximum_items)
                .map(|_| {
                    playback_envelope_value(
                        element,
                        path,
                        wire_length,
                        phase,
                        event_kind,
                        event_status,
                    )
                })
                .collect(),
        )
        .unwrap(),
        S::Variant { cases, .. } => {
            let tag = if path.starts_with("/1/") {
                event_status
            } else {
                "unsupported-control"
            };
            let case = cases.iter().find(|case| case.tag() == tag).unwrap();
            V::variant(
                ty.clone(),
                tag,
                V::leaf(case.payload_type().clone(), vec![]).unwrap(),
            )
            .unwrap()
        }
        S::Leaf(kind) => {
            let field = path.rsplit('/').next().unwrap();
            let bytes = match kind.as_str() {
                "value/bytes" => vec![0x73; wire_length],
                "value/bool" => vec![u8::from(
                    field != "terminal"
                        && (field != "native-stopped"
                            || event_kind == 3
                            || event_status == "underrun"),
                )],
                "value/u64" => {
                    let value = match field {
                        "phase" => phase,
                        "phase-kind" => event_kind,
                        "first-frame" => 2047,
                        "completed-packets" => 8,
                        "actual-bytes" => 1536,
                        other => panic!("unaccounted playback field {other}"),
                    };
                    value.to_le_bytes().to_vec()
                }
                other => panic!("unaccounted playback kind {other}"),
            };
            V::leaf(ty.clone(), bytes).unwrap()
        }
        other => panic!("unaccounted playback shape {other:?}"),
    }
}

#[test]
fn playback_retained_feedback_output_and_drain_canonical_envelopes_are_bounded() {
    let advance = prepared_playback_program("usb-audio-play-advance");
    let report = prepared_playback_program("usb-audio-play-report");
    let request = prepared_playback_program("usb-audio-play-iso-request");
    let mut advance_evaluator = PreparedPortableExpressionEvaluator::new(&advance).unwrap();
    let mut report_evaluator = PreparedPortableExpressionEvaluator::new(&report).unwrap();
    let mut request_evaluator = PreparedPortableExpressionEvaluator::new(&request).unwrap();
    for wire_length in [192, 256] {
        for (phase, kind, status) in [
            (5, 1, "running"),
            (6, 2, "completed"),
            (6, 4, "underrun"),
            (7, 3, "unsupported-control"),
        ] {
            let input =
                playback_envelope_value(&advance.input_type, "", wire_length, phase, kind, status)
                    .canonical_bytes()
                    .unwrap();
            println!(
                "playback feedback wire={wire_length} phase={phase} event={status} canonical={}",
                input.len()
            );
            assert!(
                input.len() <= 4096,
                "feedback requires a reviewed separate envelope; original PCM is retained"
            );
            let output = advance_evaluator.evaluate(&input).unwrap().to_vec();
            println!("playback retained state canonical={}", output.len());
            assert!(output.len() <= 4096);
            validate_canonical_structured_value(&output).unwrap();
            let report = report_evaluator.evaluate(&output).unwrap();
            println!("playback report canonical={}", report.len());
            assert!(report.len() <= 4096);
            let drain_state =
                playback_envelope_value(&request.input_type, "", wire_length, 7, 3, status)
                    .canonical_bytes()
                    .unwrap();
            let drain = request_evaluator.evaluate(&drain_state).unwrap();
            assert!(drain.len() <= 4096);
            let decoded = validate_canonical_structured_value(drain).unwrap();
            assert!(decoded.variant_payload("drain").unwrap().is_some());
        }
    }
}
