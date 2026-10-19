//! Installed Host evidence for the existing generic acoustic Source laws.
use conduit_audio::{AudioCycleDuration, AudioFrequencyHz, AudioLevelRatio};
use conduit_core::{
    BaseImplementationId, ConfigurationValue, ConnectionTrack, PortDirection, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("abstract acoustic control has no timer/device effect");
    }
}
#[derive(Default)]
struct Outputs(Vec<conduit_std_host::ExternalForeBytes>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
fn prepare(source: &str, entry: &str, input_port: &str) -> (StdHost, conduit_core::Plan) {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer =
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .unwrap();
        assert!(gear.accepts_realization(&offer));
        if !advertisement
            .capabilities
            .iter()
            .any(|old| old.capability_id == offer.capability_id)
        {
            advertisement.capabilities.push(offer);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, input_port),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 65536,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 65536,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&plan));
    (host, plan)
}
fn run(
    host: &mut StdHost,
    plan: &conduit_core::Plan,
    port: &str,
    inputs: Vec<Vec<u8>>,
) -> Vec<Vec<u8>> {
    let count = inputs.len();
    let inputs = inputs
        .into_iter()
        .map(|bytes| ExternalForeInput {
            front_port_id: port.into(),
            track: ConnectionTrack::Payload,
            bytes,
        })
        .collect::<Vec<_>>();
    let mut output = Outputs(Vec::with_capacity(count));
    for input in &inputs {
        // Each authored Item advances one finite ordinary Play. Caller batching
        // and fresh-host partitioning do not introduce a second runtime loop.
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                std::slice::from_ref(input),
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        let kernel = report.kernel.unwrap();
        println!(
            "acoustic-control-source={:?} plan={:?} play={:?}",
            plan.source_document_id, plan.plan_id, kernel.active_play_id
        );
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallRequested));
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    }
    assert_eq!(output.0.len(), count);
    output.0.into_iter().map(|bytes| bytes.to_vec()).collect()
}

#[test]
fn installed_frequency_cycle_conversion_retains_exact_fraction_without_sample_geometry() {
    let source = format!(
        "{}\n{}\n{}\n{}\n{}",
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../semantics/audio/decibel_types.conduit"),
        include_str!("../../../semantics/audio/acoustic_quantities.conduit"),
        ""
    );
    for (n, d) in [(440, 1), (200, 3), (1, u64::MAX), (u64::MAX, 1)] {
        let input = AudioFrequencyHz::new(d, n).unwrap();
        let (mut host, plan) = prepare(&source, "audio/frequency-to-cycle", "value");
        assert_eq!(
            plan.source_document_id,
            parse_syntax_document(&source).source_document_id()
        );
        let output = run(&mut host, &plan, "value", vec![input.encode().unwrap()]);
        let cycle = AudioCycleDuration::decode(&output[0]).unwrap();
        assert_eq!(*cycle.numerator_seconds(), d);
        assert_eq!(*cycle.denominator(), n);
        let (mut inverse_host, inverse_plan) =
            prepare(&source, "audio/cycle-to-frequency", "value");
        let inverse = run(
            &mut inverse_host,
            &inverse_plan,
            "value",
            vec![cycle.encode().unwrap()],
        );
        assert_eq!(AudioFrequencyHz::decode(&inverse[0]).unwrap(), input);
    }
}

#[test]
fn installed_db_recognition_is_exact_and_refuses_non_power_of_ten() {
    let source = format!(
        "{}\n{}\n{}\n{}\n{}",
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../semantics/audio/decibel_types.conduit"),
        include_str!("../../../semantics/audio/decibel_projection.conduit"),
        r#"plot recognize-level (
            >> value: AudioLevelRatio
            result: AudioExactPowerOfTen >>
        ) {
            working: audio/decibel-ratio-working
            recognition: audio/decibel-recognize-power-of-ten
            value >> working.value
            working.result >> recognition.value
            recognition.result >> result
        }"#
    );
    for power in [false, true] {
        for (n, d, supported, exponent, zero) in [
            (100, 1, true, 2, false),
            (1, 1000, true, -3, false),
            (7, 7, true, 0, false),
            (0, 1, true, 0, true),
            (2, 1, false, 0, false),
        ] {
            let input = if power {
                AudioLevelRatio::power(d, n)
            } else {
                AudioLevelRatio::amplitude(d, n)
            }
            .unwrap();
            let (mut host, plan) = prepare(&source, "recognize-level", "value");
            let outputs = run(&mut host, &plan, "value", vec![input.encode().unwrap()]);
            let recognition = StructuredInfoValue::from_canonical_bytes(&outputs[0]).unwrap();
            let StructuredInfoValueShape::Record(fields) = recognition.shape() else {
                panic!("recognition record")
            };
            let leaf = |name| {
                let field = fields.iter().find(|field| field.name() == name).unwrap();
                let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
                    panic!("recognition field")
                };
                bytes
            };
            assert_eq!(leaf("supported"), &[u8::from(supported)]);
            assert_eq!(leaf("zero"), &[u8::from(zero)]);
            if supported {
                assert_eq!(
                    i64::from_le_bytes(leaf("exponent").try_into().unwrap()),
                    exponent
                );
            }
        }
    }
}

#[test]
fn installed_db_projection_keeps_amplitude_and_power_conventions_distinct() {
    use conduit_audio::{
        AudioDecibelBasis, AudioDecibelConvention, AudioDecibelReference,
        AudioDecibelReferenceRole, AudioDecibelValue, AudioReferencedLevelRatio,
        AudioTrajectoryProvenance, AudioTrajectoryProvenanceKind, PreparedExactPowerOfTenDecibels,
    };
    let source = [
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../semantics/audio/decibel_types.conduit"),
        include_str!("../../../semantics/audio/decibel_projection.conduit"),
    ]
    .join("\n");
    let prepared = PreparedExactPowerOfTenDecibels::new().unwrap();
    for power in [false, true] {
        let reference = AudioDecibelReference::new(
            7,
            "declared-reference".into(),
            3,
            AudioTrajectoryProvenance::new(
                AudioTrajectoryProvenanceKind::Authored,
                "independent reference".into(),
                Some("v1".into()),
            )
            .unwrap(),
            if power {
                AudioDecibelReferenceRole::Power
            } else {
                AudioDecibelReferenceRole::AmplitudeMagnitude
            },
            conduit_core::Unit::Second,
        )
        .unwrap();
        let basis = AudioDecibelBasis::new(
            if power {
                AudioDecibelConvention::PowerTenLog10
            } else {
                AudioDecibelConvention::AmplitudeTwentyLog10
            },
            reference,
        )
        .unwrap();
        for (n, d, exponent) in [(100, 1, 2), (1, 1000, -3), (7, 7, 0)] {
            let ratio = if power {
                AudioLevelRatio::power(d, n)
            } else {
                AudioLevelRatio::amplitude(d, n)
            }
            .unwrap();
            let original = AudioReferencedLevelRatio::new(basis.clone(), ratio).unwrap();
            let receipt = prepared
                .ratio_to_decibels(&original.clone().encode().unwrap())
                .unwrap();
            assert_eq!(receipt.original(), &original);
            assert_eq!(receipt.result().basis(), &basis);
            let execution = receipt.executions().last().unwrap();
            let (mut host, plan) = prepare(&source, "audio/decibel-from-power-of-ten", "value");
            let ConfigurationValue::Text(program) =
                &plan.fragments[0].placements[0].configuration[0].value
            else {
                panic!("Source program")
            };
            assert_eq!(program, execution.source_program_hex());
            let outputs = run(
                &mut host,
                &plan,
                "value",
                vec![execution.input_canonical().to_vec()],
            );
            let value = AudioDecibelValue::decode(&outputs[0]).unwrap();
            assert_eq!(&value, receipt.result().value());
            let AudioDecibelValue::Finite(fraction) = value else {
                panic!("finite reference")
            };
            assert_eq!(
                *fraction.numerator_db(),
                exponent * if power { 10 } else { 20 }
            );
            assert_eq!(*fraction.denominator(), 1);
        }
    }
}

#[test]
fn generic_audio_cycle_flows_into_existing_speech_projection_without_a_second_quantity() {
    use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape};
    // Use the exact existing private Speech projection law, before its next
    // independent declaration. No renderer or linguistic binding is implied.
    let speech = include_str!("../../../semantics/speech/control_projection.conduit")
        .split("type SpeechAmplitudeQ15Result")
        .next()
        .unwrap();
    let source = format!(
        "{}\n{}\n{}",
        include_str!("../../../semantics/audio/types.conduit"),
        speech,
        r#"
        type AcousticCycleProjectionInput = {
            cycle: AudioCycleDuration
            sample_rate_hz: U64 in 1..=192000
        }
        plot acoustic-cycle-projection-input (
            >> value: AcousticCycleProjectionInput
            result: SpeechCycleQ8Input >>
        ) = ({numerator_seconds: .cycle.numerator_seconds, denominator: .cycle.denominator, sample_rate_hz: .sample_rate_hz})
        plot acoustic-cycle-to-speech (
            >> value: AcousticCycleProjectionInput
            result: SpeechControlQ8Result >>
        ) {
            admitted: acoustic-cycle-projection-input()
            projected: speech/cycle-q8()
            value >> admitted.value
            admitted.result >> projected.value
            projected.result >> result
        }
    "#
    );
    for (n, d) in [(1, 120), (3, 440), (1, 7)] {
        let cycle = AudioCycleDuration::new(d, n).unwrap();
        let original = cycle.encode().unwrap();
        for rate in [8000u64, 16000, 48000] {
            let (mut host, plan) = prepare(&source, "acoustic-cycle-to-speech", "value");
            let program = plan.fragments[0]
                .placements
                .iter()
                .find_map(|gear| {
                    let ConfigurationValue::Text(encoded) = &gear.configuration.first()?.value
                    else {
                        return None;
                    };
                    let program = PortableExpressionProgram::from_canonical_hex(encoded).ok()?;
                    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape()
                    else {
                        return None;
                    };
                    fields
                        .iter()
                        .any(|field| field.name() == "cycle")
                        .then_some(program)
                })
                .expect("the Source adapter consumes the generic cycle record");
            let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
                panic!("input record")
            };
            let rate_type = fields
                .iter()
                .find(|field| field.name() == "sample_rate_hz")
                .unwrap()
                .value_type()
                .clone();
            let input = StructuredInfoValue::record(
                program.input_type.clone(),
                vec![
                    StructuredFieldValue::new(
                        "cycle",
                        StructuredInfoValue::from_canonical_bytes(&original).unwrap(),
                    )
                    .unwrap(),
                    StructuredFieldValue::new(
                        "sample_rate_hz",
                        StructuredInfoValue::leaf(rate_type, rate.to_le_bytes().to_vec()).unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
            .canonical_bytes()
            .unwrap();
            let output = run(&mut host, &plan, "value", vec![input]);
            let output = StructuredInfoValue::from_canonical_bytes(&output[0]).unwrap();
            let StructuredInfoValueShape::Record(fields) = output.shape() else {
                panic!("projection record")
            };
            let number = |name| {
                let field = fields.iter().find(|field| field.name() == name).unwrap();
                let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
                    panic!("integer")
                };
                u64::from_le_bytes(bytes.try_into().unwrap())
            };
            let reference = u128::from(n) * u128::from(rate) * 256;
            assert_eq!(u128::from(number("whole_q8")), reference / u128::from(d));
            assert_eq!(
                u128::from(number("remainder_numerator")),
                reference % u128::from(d)
            );
            assert_eq!(AudioCycleDuration::decode(&original).unwrap(), cycle);
        }
    }
}
