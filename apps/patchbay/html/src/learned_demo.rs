use crate::{demonstration_snapshot, RendererSnapshot};
use patchbay_model::{
    ClockAlignment, DynamicsWatch, LearnedWatchProjection, LearnedWatchProjectionKind,
    ObjectiveComponent, ProbabilisticAlternative, ProbabilisticDisposition, ProbabilisticWatch,
    SignalContinuity, SignalPoint, SignalStreamRole, SignalWatch, StateTransition, StateWatch,
    TensorAxis, TensorWatch, TrainingPhase, TrainingWatch,
};

pub fn learned_demonstration_snapshot() -> Result<RendererSnapshot, String> {
    let mut snapshot = demonstration_snapshot()?;
    let debugger = snapshot
        .debugger
        .as_ref()
        .ok_or("learned fixture requires the authoritative debugger")?;
    let mut watches = snapshot
        .watches
        .take()
        .ok_or("learned fixture requires the authoritative Watch set")?;
    let eligible = watches.eligible_subjects.clone();
    for (subject, _) in &eligible {
        watches.add(subject).map_err(|error| format!("{error:?}"))?;
        watches
            .capture_current(subject, debugger)
            .map_err(|error| format!("{error:?}"))?;
    }
    let subject = |role| {
        eligible
            .iter()
            .find(|(_, candidate)| *candidate == role)
            .map(|(identity, _)| identity.clone())
            .ok_or_else(|| format!("learned fixture has no {role:?}"))
    };
    let gear = subject(patchbay_model::DebuggerWatchSubjectRole::Gear)?;
    let port = subject(patchbay_model::DebuggerWatchSubjectRole::Port)?;
    let cord = subject(patchbay_model::DebuggerWatchSubjectRole::Cord)?;

    project(
        &mut watches,
        &cord,
        42,
        signal(
            SignalStreamRole::AudioDerived,
            "pcm",
            "clock/audio",
            ClockAlignment::SourceClock,
            false,
        ),
    )?;
    project(
        &mut watches,
        &cord,
        42,
        LearnedWatchProjectionKind::Probabilistic(ProbabilisticWatch {
            disposition: ProbabilisticDisposition::Inferred,
            mean_milli: 360,
            standard_deviation_milli: 55,
            alternatives: vec![
                ProbabilisticAlternative {
                    label: "tongue-forward".into(),
                    value_milli: 330,
                    weight_millionths: 620_000,
                },
                ProbabilisticAlternative {
                    label: "tongue-neutral".into(),
                    value_milli: 420,
                    weight_millionths: 380_000,
                },
            ],
            sample_count: 2,
            seed_profile: "seeded/chacha8/2132".into(),
            approximation: "bounded-diagonal-posterior".into(),
            truncated: false,
        }),
    )?;
    project(
        &mut watches,
        &cord,
        42,
        LearnedWatchProjectionKind::Tensor(TensorWatch {
            dtype: "f32".into(),
            shape: vec![4, 3],
            axes: vec![
                TensorAxis {
                    role: "frame".into(),
                    unit: Some("audio-frame".into()),
                    length: 4,
                },
                TensorAxis {
                    role: "feature".into(),
                    unit: None,
                    length: 3,
                },
            ],
            total_bytes: 48,
            resource_identity: Some("checkpoint/tongues-17".into()),
            statistics_milli: Some([-510, 780, 120, 260]),
            bounded_slice_milli: vec![120, 240, -80, 310, 440, 90],
            slice_truncated: true,
        }),
    )?;

    project(
        &mut watches,
        &port,
        41,
        signal(
            SignalStreamRole::Articulatory,
            "tongue-tip-x",
            "clock/ema",
            ClockAlignment::Related {
                relation_evidence: "clock-relation/audio-ema/7".into(),
            },
            false,
        ),
    )?;
    project(
        &mut watches,
        &port,
        41,
        signal(
            SignalStreamRole::Latent,
            "z(t)/2",
            "clock/model",
            ClockAlignment::NotAligned,
            true,
        ),
    )?;
    project(
        &mut watches,
        &port,
        41,
        LearnedWatchProjectionKind::State(StateWatch {
            generation: 17,
            step: 92,
            value_identity: "recurrent-state/17".into(),
            candidate_identity: Some("recurrent-state/18".into()),
            transition: StateTransition::Committed,
            summary: "candidate 18 committed after bounded step 92".into(),
        }),
    )?;

    project(
        &mut watches,
        &gear,
        40,
        signal(
            SignalStreamRole::Metric,
            "loss/total",
            "clock/training",
            ClockAlignment::Related {
                relation_evidence: "clock-relation/model-training/3".into(),
            },
            false,
        ),
    )?;
    project(
        &mut watches,
        &gear,
        40,
        LearnedWatchProjectionKind::Training(TrainingWatch {
            phase: TrainingPhase::Training,
            split_identity: "split/train".into(),
            batch_identity: "batch/92".into(),
            step: 92,
            work_units: 4096,
            objectives: vec![
                ObjectiveComponent {
                    name: "reconstruction".into(),
                    value_milli: 184,
                },
                ObjectiveComponent {
                    name: "kl".into(),
                    value_milli: 23,
                },
            ],
            total_loss_milli: 207,
            checkpoint_event: Some("checkpoint/tongues-17-created".into()),
            pressure: Some("3 presentation updates coalesced".into()),
        }),
    )?;
    project(
        &mut watches,
        &gear,
        40,
        LearnedWatchProjectionKind::Dynamics(DynamicsWatch {
            clock_identity: "clock/model".into(),
            start_tick: 880,
            end_tick: 883,
            initial_state_milli: vec![120, -40],
            final_state_milli: vec![260, 30],
            trajectory: points(false),
            solver_work: 31,
            tolerance_millionths: 10,
            estimated_error_millionths: 7,
            truncated: false,
            refusal: None,
        }),
    )?;
    snapshot.watches = Some(watches);
    snapshot.timeline_projection = snapshot
        .timeline
        .as_ref()
        .map(|timeline| timeline.project(snapshot.watches.as_ref()));
    Ok(snapshot)
}

fn project(
    watches: &mut patchbay_model::DebuggerWatchSet,
    subject: &str,
    observation_sequence: u64,
    kind: LearnedWatchProjectionKind,
) -> Result<(), String> {
    watches
        .project_learned(
            subject,
            LearnedWatchProjection {
                observation_sequence,
                max_updates_per_second: 20,
                dropped_updates: 3,
                kind,
            },
        )
        .map_err(|error| format!("{error:?}"))
}

fn signal(
    role: SignalStreamRole,
    channel: &str,
    clock_identity: &str,
    alignment: ClockAlignment,
    gap: bool,
) -> LearnedWatchProjectionKind {
    LearnedWatchProjectionKind::Signal(SignalWatch {
        role,
        channel: channel.into(),
        unit: "normalized-milli".into(),
        clock_identity: clock_identity.into(),
        start_tick: 880,
        ticks_per_second: 200,
        continuity: if gap {
            SignalContinuity::Discontinuous
        } else {
            SignalContinuity::Continuous
        },
        alignment,
        retained_history_bytes: 4096,
        evicted_points: 12,
        points: points(gap),
    })
}

fn points(gap: bool) -> Vec<SignalPoint> {
    let mut values = vec![
        SignalPoint {
            tick: 880,
            value_milli: Some(120),
            lower_milli: Some(80),
            upper_milli: Some(160),
            disposition: ProbabilisticDisposition::Inferred,
        },
        SignalPoint {
            tick: 881,
            value_milli: Some(260),
            lower_milli: Some(210),
            upper_milli: Some(310),
            disposition: ProbabilisticDisposition::Inferred,
        },
        SignalPoint {
            tick: 882,
            value_milli: Some(190),
            lower_milli: Some(140),
            upper_milli: Some(250),
            disposition: ProbabilisticDisposition::Sampled,
        },
        SignalPoint {
            tick: 883,
            value_milli: Some(340),
            lower_milli: Some(300),
            upper_milli: Some(390),
            disposition: ProbabilisticDisposition::Observed,
        },
    ];
    if gap {
        values[2] = SignalPoint {
            tick: 882,
            value_milli: None,
            lower_milli: None,
            upper_milli: None,
            disposition: ProbabilisticDisposition::Missing,
        };
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tongues_fixture_round_trips_through_the_real_renderer_snapshot() {
        let snapshot = learned_demonstration_snapshot().unwrap();
        let encoded = snapshot.encode().unwrap();
        let decoded = RendererSnapshot::decode(&encoded, snapshot.revision).unwrap();
        let watches = decoded.watches.unwrap();
        assert_eq!(watches.watches.len(), 3);
        assert_eq!(
            watches
                .watches
                .iter()
                .map(|watch| watch.learned_projections.len())
                .sum::<usize>(),
            9
        );
    }
}
