use crate::{demonstration_snapshot, RendererSnapshot};
use patchbay_model::{
    ClockAlignment, DynamicsWatch, LearnedWatchProjection, LearnedWatchProjectionKind,
    ObjectiveComponent, ProbabilisticAlternative, ProbabilisticDisposition, ProbabilisticWatch,
    SignalContinuity, SignalPoint, SignalStreamRole, SignalWatch, StateTransition, StateWatch,
    TensorAxis, TensorWatch, TrainingPhase, TrainingWatch,
};

pub fn learned_demonstration_snapshot() -> Result<RendererSnapshot, String> {
    let report = conduit_tongues::run_research().map_err(|error| format!("{error:?}"))?;
    let corpus = conduit_tongues::Pb2007Slice::load().map_err(|error| format!("{error:?}"))?;
    let sample = corpus
        .utterances
        .iter()
        .find(|value| value.identity == report.bidirectional_query.utterance)
        .ok_or("research experiment Patchbay has no inspected utterance")?;
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
        observed_signal(
            SignalStreamRole::AudioDerived,
            "acoustic/rms",
            "clock/audio-16000hz",
            &sample
                .acoustic
                .iter()
                .map(|frame| frame[0])
                .collect::<Vec<_>>(),
        ),
    )?;
    project(
        &mut watches,
        &cord,
        42,
        LearnedWatchProjectionKind::Probabilistic(ProbabilisticWatch {
            disposition: ProbabilisticDisposition::Inferred,
            mean_milli: report.bidirectional_query.inferred_articulation.mean[0].round() as i64,
            standard_deviation_milli: report
                .bidirectional_query
                .inferred_articulation
                .standard_deviation[0]
                .round() as u64,
            alternatives: vec![
                ProbabilisticAlternative {
                    label: "conditional-sample-0".into(),
                    value_milli: report
                        .bidirectional_query
                        .inferred_articulation
                        .alternatives[0][0]
                        .round() as i64,
                    weight_millionths: 500_000,
                },
                ProbabilisticAlternative {
                    label: "conditional-sample-1".into(),
                    value_milli: report
                        .bidirectional_query
                        .inferred_articulation
                        .alternatives[1][0]
                        .round() as i64,
                    weight_millionths: 500_000,
                },
            ],
            sample_count: 2,
            seed_profile: "seeded/chacha8/2132".into(),
            approximation: "bounded-diagonal-residual-approximation".into(),
            truncated: false,
        }),
    )?;
    project(
        &mut watches,
        &cord,
        42,
        LearnedWatchProjectionKind::Tensor(TensorWatch {
            dtype: "i64".into(),
            shape: vec![16, 4],
            axes: vec![
                TensorAxis {
                    role: "frame".into(),
                    unit: Some("audio-frame".into()),
                    length: 16,
                },
                TensorAxis {
                    role: "feature".into(),
                    unit: None,
                    length: 4,
                },
            ],
            total_bytes: 512,
            resource_identity: Some(report.training.checkpoint_identity.clone()),
            statistics_milli: Some([
                sample.acoustic.iter().flatten().copied().min().unwrap_or(0),
                sample.acoustic.iter().flatten().copied().max().unwrap_or(0),
                sample.acoustic.iter().flatten().sum::<i64>() / 64,
                0,
            ]),
            bounded_slice_milli: sample.acoustic.iter().flatten().take(32).copied().collect(),
            slice_truncated: true,
        }),
    )?;

    project(
        &mut watches,
        &port,
        41,
        observed_signal(
            SignalStreamRole::Articulatory,
            "tongue-tip-x",
            "clock/ema-100hz",
            &sample
                .articulation
                .iter()
                .map(|frame| frame[0])
                .collect::<Vec<_>>(),
        ),
    )?;
    project(
        &mut watches,
        &port,
        41,
        latent_signal(
            SignalStreamRole::Latent,
            "z(t)/2",
            "clock/model",
            &report.bidirectional_query.audio_to_latent,
        ),
    )?;
    project(
        &mut watches,
        &port,
        41,
        LearnedWatchProjectionKind::State(StateWatch {
            generation: 1,
            step: report.training.steps,
            value_identity: report.training.checkpoint_identity.clone(),
            candidate_identity: Some(report.alternate_checkpoint_identity.clone()),
            transition: StateTransition::Committed,
            summary: "exact seed-2132 checkpoint committed after bounded training".into(),
        }),
    )?;

    project(
        &mut watches,
        &gear,
        40,
        observed_signal(
            SignalStreamRole::Metric,
            "objective/trajectory",
            "clock/training-step",
            &[
                (report.training.final_objectives_millionths.latent_agreement / 1_000) as i64,
                (report
                    .training
                    .final_objectives_millionths
                    .dynamics_prediction
                    / 1_000) as i64,
            ],
        ),
    )?;
    project(
        &mut watches,
        &gear,
        40,
        LearnedWatchProjectionKind::Training(TrainingWatch {
            phase: TrainingPhase::Training,
            split_identity: "split/train".into(),
            batch_identity: corpus.derivation.identity.clone(),
            step: report.training.steps,
            work_units: report.training.consumed_work_units,
            objectives: vec![
                ObjectiveComponent {
                    name: "latent-agreement".into(),
                    value_milli: (report.training.final_objectives_millionths.latent_agreement
                        / 1_000) as i64,
                },
                ObjectiveComponent {
                    name: "dynamics".into(),
                    value_milli: (report
                        .training
                        .final_objectives_millionths
                        .dynamics_prediction
                        / 1_000) as i64,
                },
            ],
            total_loss_milli: ((report.training.final_objectives_millionths.latent_agreement
                + report
                    .training
                    .final_objectives_millionths
                    .dynamics_prediction)
                / 1_000) as i64,
            checkpoint_event: Some(report.training.checkpoint_identity.clone()),
            pressure: None,
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
            initial_state_milli: report
                .bidirectional_query
                .audio_to_latent
                .iter()
                .map(|value| (value * 1_000.0).round() as i64)
                .collect(),
            final_state_milli: report
                .bidirectional_query
                .next_latent
                .iter()
                .map(|value| (value * 1_000.0).round() as i64)
                .collect(),
            trajectory: report
                .bidirectional_query
                .audio_to_latent
                .iter()
                .chain(&report.bidirectional_query.next_latent)
                .enumerate()
                .map(|(tick, value)| SignalPoint {
                    tick: tick as i64,
                    value_milli: Some((value * 1_000.0).round() as i64),
                    lower_milli: None,
                    upper_milli: None,
                    disposition: ProbabilisticDisposition::Inferred,
                })
                .collect(),
            solver_work: report.training.consumed_work_units,
            tolerance_millionths: 0,
            estimated_error_millionths: (report.held_out[1]
                .objectives_millionths
                .dynamics_prediction
                .min(u32::MAX as u64)) as u32,
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
                dropped_updates: 0,
                kind,
            },
        )
        .map_err(|error| format!("{error:?}"))
}

fn observed_signal(
    role: SignalStreamRole,
    channel: &str,
    clock_identity: &str,
    values: &[i64],
) -> LearnedWatchProjectionKind {
    LearnedWatchProjectionKind::Signal(SignalWatch {
        role,
        channel: channel.into(),
        unit: "source-microunit".into(),
        clock_identity: clock_identity.into(),
        start_tick: 0,
        ticks_per_second: 16,
        continuity: SignalContinuity::Continuous,
        alignment: ClockAlignment::SourceClock,
        retained_history_bytes: std::mem::size_of_val(values) as u32,
        evicted_points: 0,
        points: values
            .iter()
            .enumerate()
            .map(|(tick, value)| SignalPoint {
                tick: tick as i64,
                value_milli: Some(*value),
                lower_milli: None,
                upper_milli: None,
                disposition: ProbabilisticDisposition::Observed,
            })
            .collect(),
    })
}

fn latent_signal(
    role: SignalStreamRole,
    channel: &str,
    clock_identity: &str,
    values: &[f64],
) -> LearnedWatchProjectionKind {
    LearnedWatchProjectionKind::Signal(SignalWatch {
        role,
        channel: channel.into(),
        unit: "normalized-milli".into(),
        clock_identity: clock_identity.into(),
        start_tick: 0,
        ticks_per_second: 16,
        continuity: SignalContinuity::Continuous,
        alignment: ClockAlignment::Related {
            relation_evidence: "PB2007 synchronized audio/EMA derivation".into(),
        },
        retained_history_bytes: std::mem::size_of_val(values) as u32,
        evicted_points: 0,
        points: values
            .iter()
            .enumerate()
            .map(|(tick, value)| SignalPoint {
                tick: tick as i64,
                value_milli: Some((value * 1_000.0).round() as i64),
                lower_milli: None,
                upper_milli: None,
                disposition: ProbabilisticDisposition::Inferred,
            })
            .collect(),
    })
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
