use super::*;
use alloc::{format, vec};
fn legacy_plan(
    body_id: &BodyId,
    wake_id: &WakeId,
    workload_revision: u64,
    plots: &[BodyPlotPlan],
    mask_topologies: &[BodyMaskTopology],
    body_time_requirement: Option<&BodyTimeRequirement>,
) -> PlanId {
    let mut bytes = Vec::new();
    legacy_push(&mut bytes, "conduit.body/body-plan@3");
    legacy_push(&mut bytes, body_id.as_str());
    legacy_push(&mut bytes, wake_id.as_str());
    bytes.extend_from_slice(&workload_revision.to_le_bytes());
    bytes.extend_from_slice(&(plots.len() as u32).to_le_bytes());
    for plot in plots {
        legacy_push(&mut bytes, plot.plot.source_document_id.as_str());
        legacy_push(&mut bytes, plot.plot.checked_plot_id.as_str());
        legacy_push(&mut bytes, plot.plan.plan_id.as_str());
    }
    bytes.extend_from_slice(&(mask_topologies.len() as u32).to_le_bytes());
    for topology in mask_topologies {
        if let Some(plot) = &topology.face.plot {
            legacy_push(&mut bytes, "plot");
            legacy_push(&mut bytes, plot.source_document_id.as_str());
            legacy_push(&mut bytes, plot.checked_plot_id.as_str());
        } else {
            legacy_push(&mut bytes, "body");
        }
        match &topology.face.source_placement_id {
            Some(placement) => {
                legacy_push(&mut bytes, "plot-placement");
                legacy_push(&mut bytes, placement.as_str());
            }
            None => legacy_push(&mut bytes, "owner-composed"),
        }
        bytes.extend_from_slice(&(topology.chains.len() as u32).to_le_bytes());
        for chain in &topology.chains {
            legacy_push(&mut bytes, chain.plan.plan_id.as_str());
            bytes.extend_from_slice(&(chain.stage_placement_ids.len() as u32).to_le_bytes());
            for placement in &chain.stage_placement_ids {
                legacy_push(&mut bytes, placement.as_str());
            }
        }
    }
    if let Some(requirement) = body_time_requirement {
        legacy_push(&mut bytes, "body-time-requirement@1");
        legacy_push(&mut bytes, requirement.body_basis());
        bytes.extend_from_slice(&requirement.tolerance().ticks().to_le_bytes());
        legacy_scale(&mut bytes, requirement.tolerance().scale());
        bytes.extend_from_slice(&requirement.horizon().ticks().to_le_bytes());
        legacy_scale(&mut bytes, requirement.horizon().scale());
    }
    PlanId::from(legacy_digest("body-plan", &bytes))
}

fn legacy_scale(bytes: &mut Vec<u8>, scale: conduit_core::TemporalScale) {
    let label = match scale {
        conduit_core::TemporalScale::Seconds => "seconds",
        conduit_core::TemporalScale::Milliseconds => "milliseconds",
        conduit_core::TemporalScale::Microseconds => "microseconds",
        conduit_core::TemporalScale::Nanoseconds => "nanoseconds",
    };
    legacy_push(bytes, label);
}

fn legacy_play(
    body_id: &BodyId,
    wake_id: &WakeId,
    plan_id: &PlanId,
    play_sequence: u64,
) -> ActivePlayId {
    let mut bytes = Vec::new();
    legacy_push(&mut bytes, "conduit.body/body-play@1");
    legacy_push(&mut bytes, body_id.as_str());
    legacy_push(&mut bytes, wake_id.as_str());
    legacy_push(&mut bytes, plan_id.as_str());
    bytes.extend_from_slice(&play_sequence.to_le_bytes());
    ActivePlayId::from(legacy_digest("body-play", &bytes))
}

fn legacy_push(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn legacy_digest(prefix: &str, bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("{prefix}/sha256:{hex}")
}

#[test]
fn streamed_plan_and_play_preserve_legacy_canonical_bytes() {
    let body = BodyId::bound("body/fixture".into());
    let wake = WakeId::bound("wake/fixture".into());
    let mut plots = Vec::new();
    for index in 0..2 {
        let source = format!("source/{index}");
        let checked = format!("checked/{index}");
        let plan = conduit_core::seal_plan(
            conduit_core::PlotIdentity {
                source_document_id: source.clone().into(),
                checked_plot_id: checked.clone().into(),
                expanded_plot_id: format!("expanded/{index}").into(),
            },
            vec![],
        );
        plots.push(BodyPlotPlan {
            plot: ResidentPlot::new(source.into(), checked.into()),
            plan,
        });
    }
    for selected in [&plots[..0], &plots[..]] {
        for revision in [0, u64::MAX] {
            let plan = bind_body_plan(&body, &wake, revision, selected, &[], None);
            assert_eq!(
                plan,
                legacy_plan(&body, &wake, revision, selected, &[], None)
            );
            let play = bind_body_play(&body, &wake, &plan, revision);
            assert_eq!(play, legacy_play(&body, &wake, &plan, revision));
            assert_eq!(play.0.len(), "body-play/sha256:".len() + 64);
        }
    }
}

#[test]
fn streamed_topology_and_body_time_branches_match_legacy_bytes() {
    let body = BodyId::bound("body/fixture".into());
    let wake = WakeId::bound("wake/fixture".into());
    let plot = ResidentPlot::new("source/mask".into(), "checked/mask".into());
    let plan = conduit_core::seal_plan(
        conduit_core::PlotIdentity {
            source_document_id: plot.source_document_id.clone(),
            checked_plot_id: plot.checked_plot_id.clone(),
            expanded_plot_id: "expanded/mask".into(),
        },
        vec![],
    );
    let topologies = [
        BodyMaskTopology {
            face: BodyFaceSelector {
                plot: Some(plot),
                source_placement_id: Some("placement/face".into()),
            },
            chains: vec![BodyMaskChainPlan {
                plan: plan.clone(),
                stage_placement_ids: vec!["mask/a".into(), "mask/b".into()],
            }],
        },
        BodyMaskTopology {
            face: BodyFaceSelector {
                plot: None,
                source_placement_id: None,
            },
            chains: vec![BodyMaskChainPlan {
                plan,
                stage_placement_ids: vec!["mask/c".into()],
            }],
        },
    ];
    for scale in [
        conduit_core::TemporalScale::Seconds,
        conduit_core::TemporalScale::Milliseconds,
        conduit_core::TemporalScale::Microseconds,
        conduit_core::TemporalScale::Nanoseconds,
    ] {
        let requirement = conduit_core::BodyTimeRequirement::new(
            body.as_str().into(),
            conduit_core::BodyTimeTolerance::new(1, scale),
            conduit_core::MonotonicDuration::new(2, scale),
        )
        .unwrap();
        assert_eq!(
            bind_body_plan(&body, &wake, 7, &[], &topologies, Some(&requirement)),
            legacy_plan(&body, &wake, 7, &[], &topologies, Some(&requirement))
        );
    }
}

#[test]
fn borrowed_play_identity_matches_legacy_and_rejects_changed_scope() {
    for sequence in [0, 1, u64::MAX] {
        let body = BodyId::bound("body/borrowed".into());
        let wake = WakeId::bound("wake/borrowed".into());
        let plan: PlanId = "plan/borrowed".into();
        let valid = BodyPlayIdentity {
            active_play_id: bind_body_play(&body, &wake, &plan, sequence),
            body_id: body,
            wake_id: wake,
            plan_id: plan,
            play_sequence: sequence,
        };
        assert!(body_play_matches(&valid));
        for case in 0..5 {
            let mut changed = valid.clone();
            match case {
                0 => changed.body_id = BodyId::bound("body/changed".into()),
                1 => changed.wake_id = WakeId::bound("wake/changed".into()),
                2 => changed.plan_id = "plan/changed".into(),
                3 => changed.play_sequence ^= 1,
                _ => changed.active_play_id.0.push('0'),
            }
            assert!(!body_play_matches(&changed));
        }
    }
}
