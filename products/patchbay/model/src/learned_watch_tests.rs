use conduit_kernel::debug_observation::{
    DebugEventKind, DebugExecutionIdentity, DebugObservationRecord, DebugSubject,
    DEBUG_OBSERVATION_SCHEMA_VERSION, MAX_DEBUG_VALUE_PREVIEW_BYTES,
};
use conduit_kernel::CordId;

use crate::{
    ClockAlignment, DebuggerWatchBinding, DebuggerWatchError, DebuggerWatchSet,
    DebuggerWatchSubjectRole, DynamicsWatch, LearnedWatchProjection, LearnedWatchProjectionKind,
    ObjectiveComponent, ProbabilisticAlternative, ProbabilisticDisposition, ProbabilisticWatch,
    SignalContinuity, SignalPoint, SignalStreamRole, SignalWatch, StateTransition, StateWatch,
    TensorAxis, TensorWatch, TrainingPhase, TrainingWatch, MAX_LEARNED_WATCH_PROJECTIONS,
    MAX_SIGNAL_POINTS, MAX_TENSOR_SLICE_VALUES,
};

fn setup() -> DebuggerWatchSet {
    let execution = DebugExecutionIdentity {
        body: [1; 32],
        plan: [2; 32],
        play: [3; 32],
    };
    let subject = DebugSubject::Cord(CordId(7));
    let mut watches = DebuggerWatchSet::new(
        execution,
        vec![DebuggerWatchBinding {
            runtime_subject: subject,
            visible_subject: "cord/tongues".into(),
            role: DebuggerWatchSubjectRole::Cord,
        }],
    )
    .unwrap();
    watches.add("cord/tongues").unwrap();
    watches
        .observe(&DebugObservationRecord {
            schema_version: DEBUG_OBSERVATION_SCHEMA_VERSION,
            execution,
            sequence: 42,
            host_sequence: 42,
            host: 1,
            form: 1,
            subject,
            related_subject: None,
            kind: DebugEventKind::ValueSent,
            type_identity: Some(9),
            value_bytes: 2,
            preview_len: 2,
            preview_truncated: false,
            preview: {
                let mut bytes = [0; MAX_DEBUG_VALUE_PREVIEW_BYTES];
                bytes[..2].copy_from_slice(b"42");
                bytes
            },
            fault_code: None,
            causal_parent_sequence: None,
            invocation_sequence: Some(40),
        })
        .unwrap();
    watches
}

fn envelope(kind: LearnedWatchProjectionKind) -> LearnedWatchProjection {
    LearnedWatchProjection {
        observation_sequence: 42,
        max_updates_per_second: 20,
        dropped_updates: 3,
        kind,
    }
}

fn point(tick: i64, value: i64, disposition: ProbabilisticDisposition) -> SignalPoint {
    SignalPoint {
        tick,
        value_milli: Some(value),
        lower_milli: None,
        upper_milli: None,
        disposition,
    }
}

fn signal(role: SignalStreamRole, alignment: ClockAlignment) -> SignalWatch {
    SignalWatch {
        role,
        channel: "tongue-tip-x".into(),
        unit: "millimetre".into(),
        clock_identity: "clock/ema".into(),
        start_tick: 100,
        ticks_per_second: 200,
        continuity: SignalContinuity::Continuous,
        alignment,
        retained_history_bytes: 4096,
        evicted_points: 12,
        points: vec![
            point(100, 1200, ProbabilisticDisposition::Observed),
            point(101, 1400, ProbabilisticDisposition::Inferred),
        ],
    }
}

#[test]
fn one_authoritative_watch_accepts_all_finite_learned_projection_kinds() {
    let mut watches = setup();
    let projections = vec![
        LearnedWatchProjectionKind::Tensor(TensorWatch {
            dtype: "f32".into(),
            shape: vec![2, 3],
            axes: vec![
                TensorAxis {
                    role: "time".into(),
                    unit: Some("frame".into()),
                    length: 2,
                },
                TensorAxis {
                    role: "articulator".into(),
                    unit: None,
                    length: 3,
                },
            ],
            total_bytes: 24,
            resource_identity: Some("checkpoint/tongues-17".into()),
            statistics_milli: Some([-400, 900, 200, 310]),
            bounded_slice_milli: vec![100, 200, 300],
            slice_truncated: true,
        }),
        LearnedWatchProjectionKind::Signal(signal(
            SignalStreamRole::Articulatory,
            ClockAlignment::Related {
                relation_evidence: "clock-relation/audio-to-ema/7".into(),
            },
        )),
        LearnedWatchProjectionKind::Probabilistic(ProbabilisticWatch {
            disposition: ProbabilisticDisposition::Inferred,
            mean_milli: 240,
            standard_deviation_milli: 35,
            alternatives: vec![ProbabilisticAlternative {
                label: "sample/0".into(),
                value_milli: 210,
                weight_millionths: 600_000,
            }],
            sample_count: 4,
            seed_profile: "seeded/chacha8/99".into(),
            approximation: "bounded-diagonal-normal".into(),
            truncated: true,
        }),
        LearnedWatchProjectionKind::State(StateWatch {
            generation: 8,
            step: 91,
            value_identity: "state/recurrent/8".into(),
            candidate_identity: Some("state/recurrent/9".into()),
            transition: StateTransition::Committed,
            summary: "optimizer and recurrent state".into(),
        }),
        LearnedWatchProjectionKind::Training(TrainingWatch {
            phase: TrainingPhase::Training,
            split_identity: "split/train".into(),
            batch_identity: "batch/91".into(),
            step: 91,
            work_units: 4_096,
            objectives: vec![
                ObjectiveComponent {
                    name: "reconstruction".into(),
                    value_milli: 180,
                },
                ObjectiveComponent {
                    name: "kl".into(),
                    value_milli: 24,
                },
            ],
            total_loss_milli: 204,
            checkpoint_event: Some("checkpoint/tongues-17-created".into()),
            pressure: Some("telemetry-coalesced".into()),
        }),
        LearnedWatchProjectionKind::Dynamics(DynamicsWatch {
            clock_identity: "clock/model".into(),
            start_tick: 0,
            end_tick: 2,
            initial_state_milli: vec![100, 200],
            final_state_milli: vec![140, 170],
            trajectory: vec![
                point(0, 100, ProbabilisticDisposition::Observed),
                point(2, 140, ProbabilisticDisposition::Inferred),
            ],
            solver_work: 31,
            tolerance_millionths: 10,
            estimated_error_millionths: 7,
            truncated: false,
            refusal: None,
        }),
    ];
    for projection in projections {
        watches
            .project_learned("cord/tongues", envelope(projection))
            .unwrap();
    }
    let watch = &watches.watches[0];
    assert_eq!(watch.learned_projections.len(), 6);
    assert!(watch.learned_projections.iter().all(|projection| {
        projection.observation_sequence == watch.latest.as_ref().unwrap().sequence
            && projection.dropped_updates == 3
    }));
    let mut replacement = signal(
        SignalStreamRole::Articulatory,
        ClockAlignment::Related {
            relation_evidence: "clock-relation/audio-to-ema/7".into(),
        },
    );
    replacement.points[1].value_milli = Some(1_700);
    watches
        .project_learned(
            "cord/tongues",
            envelope(LearnedWatchProjectionKind::Signal(replacement)),
        )
        .unwrap();
    assert_eq!(watches.watches[0].learned_projections.len(), 6);
    assert!(watches.watches[0]
        .learned_projections
        .iter()
        .any(|projection| {
            matches!(&projection.kind, LearnedWatchProjectionKind::Signal(signal)
            if signal.role == SignalStreamRole::Articulatory
                && signal.points[1].value_milli == Some(1_700))
        }));
}

#[test]
fn missing_samples_and_unrelated_clocks_remain_explicit() {
    let mut watches = setup();
    let mut value = signal(SignalStreamRole::AudioDerived, ClockAlignment::NotAligned);
    value.continuity = SignalContinuity::Discontinuous;
    value.points.push(SignalPoint {
        tick: 102,
        value_milli: None,
        lower_milli: None,
        upper_milli: None,
        disposition: ProbabilisticDisposition::Missing,
    });
    watches
        .project_learned(
            "cord/tongues",
            envelope(LearnedWatchProjectionKind::Signal(value)),
        )
        .unwrap();
    let encoded = serde_json::to_string(&watches).unwrap();
    assert!(encoded.contains("not-aligned"));
    assert!(encoded.contains("discontinuous"));
    assert!(encoded.contains("missing"));
}

#[test]
fn bounds_bad_alignment_and_stale_projection_are_refused() {
    let mut watches = setup();
    let mut oversized = signal(SignalStreamRole::Latent, ClockAlignment::SourceClock);
    oversized.points = (0..=MAX_SIGNAL_POINTS)
        .map(|index| {
            point(
                index as i64,
                index as i64,
                ProbabilisticDisposition::Inferred,
            )
        })
        .collect();
    assert_eq!(
        watches.project_learned(
            "cord/tongues",
            envelope(LearnedWatchProjectionKind::Signal(oversized)),
        ),
        Err(DebuggerWatchError::InvalidProjection)
    );

    let mut stale = envelope(LearnedWatchProjectionKind::Signal(signal(
        SignalStreamRole::Latent,
        ClockAlignment::Related {
            relation_evidence: String::new(),
        },
    )));
    assert_eq!(
        watches.project_learned("cord/tongues", stale.clone()),
        Err(DebuggerWatchError::InvalidProjection)
    );
    stale.kind = LearnedWatchProjectionKind::Tensor(TensorWatch {
        dtype: "f32".into(),
        shape: vec![MAX_TENSOR_SLICE_VALUES as u32 + 1],
        axes: vec![TensorAxis {
            role: "feature".into(),
            unit: None,
            length: MAX_TENSOR_SLICE_VALUES as u32 + 1,
        }],
        total_bytes: 132,
        resource_identity: None,
        statistics_milli: None,
        bounded_slice_milli: vec![0; MAX_TENSOR_SLICE_VALUES + 1],
        slice_truncated: false,
    });
    assert_eq!(
        watches.project_learned("cord/tongues", stale),
        Err(DebuggerWatchError::InvalidProjection)
    );

    for index in 0..MAX_LEARNED_WATCH_PROJECTIONS {
        let mut value = signal(SignalStreamRole::Metric, ClockAlignment::SourceClock);
        value.channel = format!("metric/{index}");
        watches
            .project_learned(
                "cord/tongues",
                envelope(LearnedWatchProjectionKind::Signal(value)),
            )
            .unwrap();
    }
    let mut overflow = signal(SignalStreamRole::Metric, ClockAlignment::SourceClock);
    overflow.channel = "metric/overflow".into();
    assert_eq!(
        watches.project_learned(
            "cord/tongues",
            envelope(LearnedWatchProjectionKind::Signal(overflow)),
        ),
        Err(DebuggerWatchError::ProjectionLimit)
    );

    let mut wrong_sequence = envelope(LearnedWatchProjectionKind::Signal(signal(
        SignalStreamRole::Metric,
        ClockAlignment::SourceClock,
    )));
    wrong_sequence.observation_sequence = 41;
    watches.clear_history("cord/tongues").unwrap();
    assert_eq!(
        watches.project_learned("cord/tongues", wrong_sequence),
        Err(DebuggerWatchError::InvalidProjection)
    );
}
