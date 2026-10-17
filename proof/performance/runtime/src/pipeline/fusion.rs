//! Physical dispatch alternative; cost inputs are explicit fixture assumptions.
use super::*;
use conduit_planner::*;

pub(super) fn select(
    authoring: &conduit_plot::ExpandedAuthoringPlot,
    hosts: &[conduit_core::HostAdvertisement],
    placements: &PlacementChoices,
    fused_work: u64,
    boundaries: &[FusionBoundary],
) -> Result<FusionSelection, Box<dyn Error>> {
    let plot = &authoring.expanded;
    let host = &hosts[0];
    let (offer, kinds) = conduit_std_host::pure_chain_fusion::pure_chain_fusion_offer(plot, host)?;
    let provenance = |id: &str| ObservationProvenance {
        sign_id: format!("sign/runtime-chain/{id}").into(),
        source: "frozen dispatch work assumptions; runtime counters separately measured".into(),
        observed_at_ms: 0,
        valid_until_ms: 0,
    };
    let basis = LocalityPlanningBasis {
        now_ms: 0,
        horizon_seconds: 1,
        remote_bytes_per_second_ceiling: Some(0),
        data_flow: DataFlowObservation {
            source_gear_id: offer.gear_ids[0].clone(),
            items_per_second: ITEMS as u64,
            bytes_per_item: 8,
            provenance: provenance("flow"),
        },
        reductions: vec![],
        transports: vec![],
        resources: vec![],
        realization_work: plot
            .gears
            .iter()
            .map(|g| RealizationWorkObservation {
                gear_id: g.gear_id.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                capability_id: placements.by_gear[&g.gear_id].capability_id.clone(),
                work_units: (ADDS / plot.gears.len() + 1) as u64,
                provenance: provenance(g.gear_id.as_str()),
            })
            .collect(),
        // This Back deliberately materializes every Cord. Set removable local
        // transport savings to zero rather than claiming avoided encoding/copies.
        local_cords: plot
            .connections
            .iter()
            .map(|c| LocalCordObservation {
                source_gear_id: c.source_gear_id.clone(),
                sink_gear_id: c.sink_gear_id.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                work_units: 0,
                provenance: provenance(&format!("cord/{}", c.source_gear_id.as_str())),
            })
            .collect(),
    };
    let candidates: Vec<_> = [false, true]
        .into_iter()
        .map(|fused| {
            let id = if fused { "fused" } else { "unfused" };
            FusionCandidate {
                candidate_id: id.into(),
                realization: LocalityCandidate {
                    candidate_id: id.into(),
                    placements: placements.clone(),
                    lines: BTreeMap::new(),
                },
                fusion_ids: if fused {
                    vec![offer.fusion_id.clone()]
                } else {
                    vec![]
                },
            }
        })
        .collect();
    let observation = FusionPlanningObservation {
        fusion_id: offer.fusion_id.clone(),
        fused_work_units: ADDS as u64 + fused_work,
        provenance: provenance("fused"),
    };
    Ok(select_expanded_fusion_candidate(
        plot,
        hosts,
        &candidates,
        &basis,
        FusionPlanningInputs {
            semantic_kinds: &kinds,
            offers: &[offer],
            observations: &[observation],
            boundaries,
            line_offers: &[],
        },
    )?)
}
