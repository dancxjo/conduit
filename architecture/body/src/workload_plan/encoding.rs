//! One historical Body Plan canonical encoder for both preparation representations.
use super::*;
pub(super) fn bind_body_plan_views<'a>(
    body_id: &BodyId,
    wake_id: &WakeId,
    workload_revision: u64,
    plots: impl ExactSizeIterator<Item = views::PlotRef<'a>>,
    mask_topologies: impl ExactSizeIterator<Item = views::MaskRef<'a>>,
    body_time_requirement: Option<&BodyTimeRequirement>,
) -> PlanId {
    let mut hash = Sha256::new();
    push(&mut hash, "conduit.body/body-plan@3");
    push(&mut hash, body_id.as_str());
    push(&mut hash, wake_id.as_str());
    hash.update(workload_revision.to_le_bytes());
    hash.update((plots.len() as u32).to_le_bytes());
    for plot in plots {
        push(&mut hash, plot.plot.source_document_id.as_str());
        push(&mut hash, plot.plot.checked_plot_id.as_str());
        push(&mut hash, plot.plan.metadata().plan_id.as_str());
    }
    hash.update((mask_topologies.len() as u32).to_le_bytes());
    for topology in mask_topologies {
        if let Some(plot) = &topology.face.plot {
            push(&mut hash, "plot");
            push(&mut hash, plot.source_document_id.as_str());
            push(&mut hash, plot.checked_plot_id.as_str());
        } else {
            push(&mut hash, "body");
        }
        match &topology.face.source_placement_id {
            Some(placement) => {
                push(&mut hash, "plot-placement");
                push(&mut hash, placement.as_str());
            }
            None => push(&mut hash, "owner-composed"),
        }
        hash.update((topology.chains().len() as u32).to_le_bytes());
        for chain in topology.chains() {
            push(&mut hash, chain.plan.metadata().plan_id.as_str());
            hash.update((chain.stages.len() as u32).to_le_bytes());
            for placement in chain.stages {
                push(&mut hash, placement.as_str());
            }
        }
    }
    if let Some(requirement) = body_time_requirement {
        push(&mut hash, "body-time-requirement@1");
        push(&mut hash, requirement.body_basis());
        hash.update(requirement.tolerance().ticks().to_le_bytes());
        push_temporal_scale(&mut hash, requirement.tolerance().scale());
        hash.update(requirement.horizon().ticks().to_le_bytes());
        push_temporal_scale(&mut hash, requirement.horizon().scale());
    }
    PlanId::from(digest_id("body-plan", hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    mod fixture {
        use alloc::{vec, vec::Vec};
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../core/tests/common/sealed_state.rs"
        ));
    }
    #[test]
    fn historical_body_plan_canonical_cases() {
        let core = fixture::seal(fixture::fragment());
        let plot = ResidentPlot::new(
            core.source_document_id.clone(),
            core.checked_plot_id.clone(),
        );
        let (_, wake) =
            crate::Body::born_with_plots(BodyWorkset::one(plot.clone()).unwrap(), 1, "born".into())
                .unwrap()
                .wake(2, "wake".into())
                .unwrap();
        let plots = [BodyPlotPlan {
            plot: plot.clone(),
            plan: core.clone(),
        }];
        let scales = [
            conduit_core::TemporalScale::Seconds,
            conduit_core::TemporalScale::Milliseconds,
            conduit_core::TemporalScale::Microseconds,
            conduit_core::TemporalScale::Nanoseconds,
        ];
        // Frozen from the historical buffered encoder before streaming.
        let expected = [
            (
                "body-plan/sha256:2f4ce1fbe15375a9497de867874e3e081531ca8c46ac3f1a70ad1f40b997ae10",
                "body-play/sha256:2d10c8d732e213565e5620f7a7d6285898b8c52e316103ef81621bbdf36f62b7",
            ),
            (
                "body-plan/sha256:90dab98e63a0d128c7d0c0904c0b01946c2a536ac898ef6663b2dedb208f1689",
                "body-play/sha256:181d5588f9d6ad30a3d35d64f5cd03207de01731df95d84da126b9db09c74485",
            ),
            (
                "body-plan/sha256:e52f9a6693610385990be390d4c8bb3b63a07fa58e279f1a74af5c4b797f9608",
                "body-play/sha256:47c982c8b148214f59f08ef06be6eb2e39844019b5e64df30cf218ec1430f125",
            ),
            (
                "body-plan/sha256:f032255caa9290f9565719d4309aba657a143560f5d1c697a64a79d2f3152b6d",
                "body-play/sha256:3076a8d6a439640e781c07c9bd9d5b3aec8253228a77fb55f1a5bbe37a80e14f",
            ),
            (
                "body-plan/sha256:b02c922d943d17ccef7a5d05cfce75cdfa7acb9221fe43398c9cdb7a74cf7463",
                "body-play/sha256:d1deea9978919a16d5d4310999d8d21c16b10743d7c83204ede3da0a2c4520f6",
            ),
            (
                "body-plan/sha256:e33c91c1f5d45d9386c7e07ba6012a10e47ce1d88792417307b9a85a188ea410",
                "body-play/sha256:39710d52866939adc69d55e9e7e57889b6934da1b5ff820d11d1acfbf9bc8cd2",
            ),
        ];
        for case in 0..6 {
            let masks = if case == 0 {
                vec![]
            } else {
                vec![BodyMaskTopology {
                    face: BodyFaceSelector {
                        plot: (case >= 2).then(|| plot.clone()),
                        source_placement_id: (case >= 3).then(|| "placement".into()),
                    },
                    chains: vec![BodyMaskChainPlan {
                        plan: core.clone(),
                        stage_placement_ids: vec!["placement".into()],
                    }],
                }]
            };
            let time = (case >= 2).then(|| {
                BodyTimeRequirement::new(
                    wake.body_id.as_str().into(),
                    conduit_core::BodyTimeTolerance::new(2, scales[case - 2]),
                    conduit_core::MonotonicDuration::new(500, scales[case - 2]),
                )
                .unwrap()
            });
            let id = bind_body_plan_views(
                &wake.body_id,
                &wake.wake_id,
                wake.workload_revision,
                plots.iter().map(views::PlotRef::rich),
                masks.iter().map(views::MaskRef::rich),
                time.as_ref(),
            );
            assert_eq!(id.as_str(), expected[case].0);
            assert_eq!(
                bind_body_play(&wake.body_id, &wake.wake_id, &id, 37).as_str(),
                expected[case].1
            );
        }
    }
}
