use conduit_audio::*;
use conduit_core::{
    ConfigurationValue, StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
struct Pipeline {
    programs: Vec<PortableExpressionProgram>,
    prepared: Vec<PreparedPortableExpressionEvaluator>,
}
impl Pipeline {
    fn new(entry: &str, input: StructuredInfoType) -> Self {
        let source = [
            include_str!("../types.conduit"),
            include_str!("../trajectory_types.conduit"),
            include_str!("../rate_types.conduit"),
            include_str!("../decibel_types.conduit"),
            include_str!("../exact_control_types.conduit"),
            include_str!("../time_analysis_types.conduit"),
            include_str!("../dsp/periodic-types.conduit"),
            PERIODIC_FLOW_SOURCE,
            PERIODIC_OSCILLATOR_SOURCE,
            PERIODIC_VALIDATION_SOURCE,
        ]
        .join("\n");
        let syntax = parse_syntax_document(&source);
        assert_eq!(syntax.round_trip(), source);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
        let mut unordered: Vec<_> = expanded
            .expanded
            .gears
            .iter()
            .map(|gear| {
                let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
                    panic!("program")
                };
                (
                    gear.gear_id.clone(),
                    PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                )
            })
            .collect();
        let mut ty = input;
        let mut programs = Vec::new();
        while !unordered.is_empty() {
            let index = unordered
                .iter()
                .position(|(id, p)| {
                    p.input_type == ty
                        && !expanded.expanded.connections.iter().any(|edge| {
                            edge.sink_gear_id == *id
                                && unordered
                                    .iter()
                                    .any(|(upstream, _)| *upstream == edge.source_gear_id)
                        })
                })
                .expect("one linear typed pipeline");
            let (_, program) = unordered.remove(index);
            ty = program.output_type.clone();
            programs.push(program);
        }
        let prepared = programs
            .iter()
            .map(|p| PreparedPortableExpressionEvaluator::new(p).unwrap())
            .collect();
        Self { programs, prepared }
    }
    fn run(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        for (program, prepared) in self.programs.iter().zip(&mut self.prepared) {
            let reference = program.evaluate(&bytes).unwrap();
            assert_eq!(prepared.evaluate(&bytes).unwrap(), reference);
            bytes = reference;
        }
        bytes
    }
}
fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn provenance() -> AudioTrajectoryProvenance {
    AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "periodic fixture".into(),
        None,
    )
    .unwrap()
}
fn request(
    n: u64,
    d: u64,
    phase: i64,
    phase_den: u64,
    amp_n: u64,
    amp_d: u64,
    frames: u32,
) -> AudioPeriodicRequest {
    AudioPeriodicRequest::new(
        AudioRelativeAmplitude::new(amp_d, amp_n).unwrap(),
        AudioSampleRateBasis::new(anchor(), AudioFrameQuantization::Floor, 8000).unwrap(),
        frames,
        AudioFrequencyControl::new(anchor(), provenance(), AudioFrequencyHz::new(d, n).unwrap())
            .unwrap(),
        AudioPhaseReference::new(
            anchor(),
            AudioExactTimeOffset::new(1, 0).unwrap(),
            phase_den,
            phase,
            provenance(),
        )
        .unwrap(),
        AudioPeriodicReference::UnscaledCubicFlow,
    )
    .unwrap()
}
fn started(bytes: &[u8]) -> AudioPeriodicCursorCandidate {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("start outcome")
    };
    assert_eq!(tag, "started");
    AudioPeriodicCursorCandidate::decode(&payload.canonical_bytes().unwrap()).unwrap()
}
#[test]
fn full_existing_period_profile_and_fractional_cycle_keep_exact_request_and_remainders() {
    let mut start = Pipeline::new(
        "dsp/oscillator-prepare",
        AudioPeriodicRequest::semantic_type().unwrap(),
    );
    assert_eq!(start.programs.len(), 4);
    for period in 58..=69 {
        let req = request(8000, period, 0, 1, 1, 1, 4);
        let cursor = started(&start.run(req.clone().encode().unwrap()));
        let projection = cursor.profile().projection();
        assert_eq!(*projection.request(), req);
        assert_eq!(*projection.period_q8(), i128::from(period) * 256);
        assert_eq!(*projection.period_remainder(), 0);
        assert_eq!(*projection.amplitude_q15(), 32768);
        assert_eq!(*cursor.phase_q8(), 0);
    }
    let req = request(120, 1, -1, 3, 1, 3, 4);
    let cursor = started(&start.run(req.clone().encode().unwrap()));
    let projection = cursor.profile().projection();
    assert_eq!(*projection.request(), req);
    assert_eq!(
        (*projection.period_q8(), *projection.period_remainder()),
        (17066, 80)
    );
    assert_eq!(
        (
            *projection.amplitude_q15(),
            *projection.amplitude_remainder()
        ),
        (10922, 2)
    );
    assert_eq!(
        (*cursor.initial_phase_q8(), *cursor.phase_remainder()),
        (11377, 1)
    );
    assert_eq!(*projection.request().phase().numerator_turns(), -1);
}
#[test]
fn wide_originals_and_unsupported_domains_refuse_before_phase_multiplication() {
    let mut start = Pipeline::new(
        "dsp/oscillator-prepare",
        AudioPeriodicRequest::semantic_type().unwrap(),
    );
    let scale = u64::MAX / 8000;
    let req = request(8000 * scale, 64 * scale, 7, 4, u64::MAX, u64::MAX, 1);
    let cursor = started(&start.run(req.clone().encode().unwrap()));
    assert_eq!(*cursor.profile().projection().request(), req);
    assert_eq!(*cursor.profile().projection().period_q8(), 16384);
    assert_eq!(*cursor.initial_phase_q8(), 12288);
    for (req, expected) in [
        (
            request(1, u64::MAX, i64::MIN, u64::MAX, 1, 1, 1),
            "unsupported_frequency",
        ),
        (request(u64::MAX, 1, 0, 1, 1, 1, 1), "unsupported_frequency"),
        (
            request(125, 1, 0, 1, 131073, 65536, 1),
            "unsupported_amplitude",
        ),
    ] {
        let bytes = start.run(req.encode().unwrap());
        let value = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
            panic!("outcome")
        };
        assert_eq!(tag, expected);
    }
}
#[test]
fn shared_pulse_steps_keep_known_samples_finite_budget_and_cancelled_cursor() {
    let mut start = Pipeline::new(
        "dsp/oscillator-prepare",
        AudioPeriodicRequest::semantic_type().unwrap(),
    );
    let req = request(125, 1, 0, 1, 1, 1, 3);
    let mut cursor = started(&start.run(req.encode().unwrap()));
    let original = cursor.clone();
    let mut step = Pipeline::new(
        "dsp/oscillator-step",
        AudioPeriodicStepInput::semantic_type().unwrap(),
    );
    assert_eq!(step.programs.len(), 23);
    for sample in [0, 24, 72] {
        let bytes = step.run(
            AudioPeriodicStepInput::new(false, cursor)
                .unwrap()
                .encode()
                .unwrap(),
        );
        let AudioPeriodicOutcome::Advanced(value) = AudioPeriodicOutcome::decode(&bytes).unwrap()
        else {
            panic!("advanced")
        };
        assert_eq!(*value.sample(), sample);
        cursor = value.cursor().clone();
    }
    assert_eq!(
        (*cursor.emitted(), *cursor.remaining(), *cursor.phase_q8()),
        (3, 0, 768)
    );
    let bytes = step.run(
        AudioPeriodicStepInput::new(false, cursor.clone())
            .unwrap()
            .encode()
            .unwrap(),
    );
    assert!(matches!(
        AudioPeriodicOutcome::decode(&bytes).unwrap(),
        AudioPeriodicOutcome::Exhausted(_)
    ));
    let bytes = step.run(
        AudioPeriodicStepInput::new(true, original.clone())
            .unwrap()
            .encode()
            .unwrap(),
    );
    let value = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("cancelled")
    };
    assert_eq!(tag, "cancelled");
    assert_eq!(
        AudioPeriodicCursorCandidate::decode(&payload.canonical_bytes().unwrap()).unwrap(),
        original
    );
    let raw = original.profile().projection();
    let projection = AudioPeriodicProjection::new(
        *raw.amplitude_q15(),
        *raw.amplitude_remainder(),
        *raw.period_q8(),
        *raw.period_remainder(),
        raw.request().clone(),
    )
    .unwrap();
    let profile = AudioPeriodicProfile::new(projection).unwrap();
    assert!(AudioPeriodicCursor::new(0, 0, 1, 0, profile, 3).is_err());
}

#[test]
fn complete_cycles_match_independent_integer_reference_and_reset() {
    let mut start = Pipeline::new(
        "dsp/oscillator-prepare",
        AudioPeriodicRequest::semantic_type().unwrap(),
    );
    let mut step = Pipeline::new(
        "dsp/oscillator-step",
        AudioPeriodicStepInput::semantic_type().unwrap(),
    );
    for (frequency, denominator, phase, phase_denominator, amplitude, amplitude_denominator) in [
        (8000, 58, 0, 1, 1, 1),
        (125, 1, -1, 3, 1, 3),
        (8000, 69, 7, 4, 2, 1),
        (120, 1, -7, 5, 1, 2),
    ] {
        let frames = 150;
        let req = request(
            frequency,
            denominator,
            phase,
            phase_denominator,
            amplitude,
            amplitude_denominator,
            frames,
        );
        let initial = started(&start.run(req.clone().encode().unwrap()));
        let period = i128::from(denominator) * 8000 * 256 / i128::from(frequency);
        let initial_phase = i128::from(phase).rem_euclid(i128::from(phase_denominator)) * period
            / i128::from(phase_denominator);
        let gain = i128::from(amplitude) * 32768 / i128::from(amplitude_denominator);
        let open = (period * 3 / 4 / 256) * 256;
        let reference_flow = |phase: i128| {
            let q = phase * 256 / open;
            if (1..256).contains(&q) {
                q * q * (256 - q) / 256
            } else {
                0
            }
        };
        let mut cursor = initial.clone();
        for index in 0..frames {
            let phase = (initial_phase + i128::from(index) * 256) % period;
            let previous = if phase < 256 {
                0
            } else {
                reference_flow(phase - 256)
            };
            let expected = (reference_flow(phase) - previous) * gain / 32768;
            let bytes = step.run(
                AudioPeriodicStepInput::new(false, cursor)
                    .unwrap()
                    .encode()
                    .unwrap(),
            );
            let AudioPeriodicOutcome::Advanced(value) =
                AudioPeriodicOutcome::decode(&bytes).unwrap()
            else {
                panic!("advanced")
            };
            assert_eq!(
                *value.sample(),
                expected,
                "frequency {frequency}/{denominator}, frame {index}"
            );
            cursor = value.cursor().clone();
            assert_eq!(
                *cursor.phase_q8(),
                (initial_phase + i128::from(index + 1) * 256) % period
            );
            assert_eq!(*cursor.remaining(), frames - index - 1);
            // Every frame crosses a canonical cursor boundary; caller partitioning
            // therefore cannot introduce an alternate recurrence state.
            cursor = AudioPeriodicCursorCandidate::decode(&cursor.encode().unwrap()).unwrap();
        }
        assert_eq!(started(&start.run(req.encode().unwrap())), initial);
    }
}
