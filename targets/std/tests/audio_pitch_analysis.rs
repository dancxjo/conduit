use conduit_audio::*;
use conduit_core::{BaseImplementationId, ConnectionTrack, PortDirection};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
#[derive(Default)]
struct Output(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Output {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value.bytes);
        Ok(())
    }
}
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("pure analysis cannot wait")
    }
}
fn checked_plan() -> (StdHost, conduit_core::Plan) {
    checked_plan_for(include_str!(
        "../../../plots/audio-pitch-analysis/main.conduit"
    ))
}
fn checked_plan_for(source: &str) -> (StdHost, conduit_core::Plan) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_audio_pitch_analysis_catalog(&mut startup, &mut profile)
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "measure", &profile).unwrap();
    let host = StdHost::new();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "pcm"),
        (PortDirection::Output, "observation"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
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
    assert_eq!(
        plan.fragments[0].placements[0].implementation_id.as_str(),
        conduit_std_offers::AUDIO_PITCH_ANALYSIS_IMPLEMENTATION
    );
    (host, plan)
}
fn pcm(values: &[f32], clock: u64, rate: u32, discontinuity: bool) -> Vec<u8> {
    let payload: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        rate,
        PcmChannelLayout::Mono,
        values.len() as u16,
        clock,
        123,
        discontinuity,
    )
    .unwrap()
    .encode_frame(&payload)
    .unwrap()
}
#[test]
fn ordinary_source_plan_play_emits_four_observation_states_with_exact_custody() {
    let (mut host, plan) = checked_plan();
    let mut seed = 42u32;
    let noise: Vec<f32> = (0..1024)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 16) as f32 / 65536.0 - 0.5
        })
        .collect();
    let tone = |period: f64, weak: f64, harmonic: f64| {
        (0..1024)
            .map(|i| {
                let phase = std::f64::consts::TAU * i as f64 / period;
                (weak * phase.sin() + harmonic * (2.0 * phase).sin()) as f32
            })
            .collect::<Vec<_>>()
    };
    let samples = [
        vec![0.0; 1024],
        noise,
        tone(80.0, 0.000001, 0.5),
        tone(40.0, 0.5, 0.0),
    ];
    let states = [
        AudioPitchState::UnknownInsufficientEnergy,
        AudioPitchState::Unknown,
        AudioPitchState::Uncertain,
        AudioPitchState::Periodic,
    ];
    let profile = PcmPitchProfile::new(7, 8000, 1024, 0, 1024, 8, 256).unwrap();
    let mut expected = Vec::new();
    let inputs: Vec<_> = samples
        .iter()
        .enumerate()
        .map(|(index, samples)| {
            let bytes = PcmFrameHeader::new(
                PcmSampleRepresentation::Float32LittleEndian,
                8000,
                PcmChannelLayout::Mono,
                1024,
                7,
                123 + index as u64 * 1024,
                false,
            )
            .unwrap()
            .encode_frame(
                &samples
                    .iter()
                    .flat_map(|x| x.to_le_bytes())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            expected.push(
                profile
                    .observe(&mut PcmPitchWorkspace::default(), &bytes)
                    .unwrap()
                    .prepare_native()
                    .unwrap(),
            );
            ExternalForeInput {
                front_port_id: "pcm".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }
        })
        .collect();
    let mut output = Output::default();
    let report = host
        .run_external_plot_sequence_to(
            plan.fragments[0].clone(),
            &inputs,
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    let actual: Vec<_> = output
        .0
        .iter()
        .map(|row| AudioPitchObservation::decode(row).unwrap())
        .collect();
    assert_eq!(actual, expected);
    for (index, (observation, state)) in actual.iter().zip(states).enumerate() {
        use conduit_core::projection::*;
        assert_eq!(*observation.state(), state);
        assert_eq!(*observation.source_start_frame(), 123 + index as u64 * 1024);
        assert_eq!(
            *observation.source_available_frame(),
            123 + (index as u64 + 1) * 1024
        );
        let domain = PcmPitchEvidence::bind(profile, &inputs[index].bytes, observation).unwrap();
        let facts = domain.facts();
        let native = domain.native();
        let placement = &plan.fragments[0].placements[0];
        let realization = ProjectionRealization {
            plan: &plan.plan_id,
            host: &placement.host_id,
            boot: &placement.boot_id,
            back: &placement.implementation_id,
            artifact: &placement.artifact_id,
        };
        let admitted = [realization];
        let attempts = [ProjectionAttempt {
            realization,
            outcome: ProjectionMechanism::Completed,
            fallback_reason: None,
        }];
        let text = |s| ProjectionText::new(s).unwrap();
        let policy = PcmPitchObservationPolicy;
        let fidelity = ProjectionReport::new(
            &domain,
            &policy,
            ProjectionInput {
                basis: ProjectionBasis {
                    report: text("actual-pitch-output"),
                    source: text("retained-pcm"),
                    target: Some(text("emitted-observation")),
                    projector: text(PCM_PITCH_PROFILE),
                    requested_route: text("observed-periodicity"),
                    boundary: None,
                },
                source: domain.source(),
                target: Some(observation),
                facts: &facts,
                native: &native,
                scores: &[],
                admitted: &admitted,
                attempts: &attempts,
                selected_attempt: Some(0),
                mechanism: ProjectionMechanism::Completed,
                diagnostics: &[],
            },
        )
        .unwrap();
        assert!(fidelity.require_exact().is_err());
    }
    let kernel = report.kernel.expect("production kernel");
    assert_eq!(kernel.active_play.plan_id, plan.plan_id);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::StepFuelGranted));
    assert!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::ValueConsumed)
            .count()
            >= 4
    );
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::BackCompleted));
}

#[test]
fn normal_kernel_refuses_malformed_whole_input_before_output() {
    let mut nonfinite = vec![0.0; 1024];
    nonfinite[1023] = f32::NAN;
    for bytes in [
        pcm(&vec![0.0; 1024], 8, 8000, false),
        pcm(&vec![0.0; 1024], 7, 16000, false),
        pcm(&nonfinite, 7, 8000, false),
        pcm(&vec![0.0; 512], 7, 8000, false),
        pcm(&vec![0.0; 1024], 7, 8000, true),
    ] {
        let (mut host, plan) = checked_plan();
        let mut output = Output::default();
        let result = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "pcm".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        );
        assert!(result.is_err());
        assert!(output.0.is_empty());
    }
}

// Accepted Audio-owned authored-generation fixture, separate from the installed
// measurement path. This does not claim a complete streaming synth composition.
#[path = "../../../semantics/audio/tests/common/periodic.rs"]
mod oscillator_fixture;
#[test]
fn accepted_authored_oscillator_samples_feed_the_same_normal_measurement_kind() {
    use oscillator_fixture::{request, started, Pipeline};
    let mut prepare = Pipeline::new(
        "dsp/oscillator-prepare",
        AudioPeriodicRequest::semantic_type().unwrap(),
    );
    let mut step = Pipeline::new(
        "dsp/oscillator-step",
        AudioPeriodicStepInput::semantic_type().unwrap(),
    );
    let mut cursor = started(&prepare.run(request(1000, 1, 0, 1, 1, 1, 64).encode().unwrap()));
    assert_eq!(*cursor.profile().projection().period_q8(), 8 * 256);
    let mut samples = Vec::with_capacity(64);
    for _ in 0..64 {
        let bytes = step.run(
            AudioPeriodicStepInput::new(false, cursor)
                .unwrap()
                .encode()
                .unwrap(),
        );
        let AudioPeriodicOutcome::Advanced(value) = AudioPeriodicOutcome::decode(&bytes).unwrap()
        else {
            panic!("bounded authored sample");
        };
        let sample = i32::try_from(*value.sample()).unwrap();
        assert!((-32768..=32767).contains(&sample));
        // Explicit fixture conversion: one unscaled cubic-difference integer
        // becomes 1/32768 digital full scale. This dyadic map is exact in f32.
        samples.push(sample as f32 / 32768.0);
        cursor = value.cursor().clone();
    }
    let source = include_str!("../../../plots/audio-pitch-analysis/main.conduit")
        .replace("clock-id = 7", "clock-id = 1")
        .replace("source-frames = 1024", "source-frames = 64")
        .replace("window-frames = 1024", "window-frames = 64")
        .replace("minimum-lag = 8", "minimum-lag = 2")
        .replace("maximum-lag = 256", "maximum-lag = 16");
    let (mut host, plan) = checked_plan_for(&source);
    let bytes = PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        8000,
        PcmChannelLayout::Mono,
        64,
        1,
        0,
        false,
    )
    .unwrap()
    .encode_frame(
        &samples
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let mut output = Output::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "pcm".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(output.0.len(), 1);
    let observed = AudioPitchObservation::decode(&output.0[0]).unwrap();
    assert_eq!(*observed.state(), AudioPitchState::Periodic);
    assert_eq!(*observed.candidate_lag(), 8);
    assert_eq!(*observed.window_start_frame(), 0);
    assert_eq!(report.kernel.unwrap().active_play.plan_id, plan.plan_id);
}
