//! A named Host offers exact owner layouts; the generic Planner makes selection.
use super::*;
use conduit_ai::fixed_numeric_compact::*;
use conduit_planner::*;
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn prepare(
    plot: &conduit_plot::CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    mode: u8,
) -> Plan {
    // Local maps must outlive the complete planning call.
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &connection_bases,
        line_candidates: &line_candidates,
        connection_item_capacity: 1,
        connection_byte_capacity: 16384,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    if mode == 0 {
        return plan_with_options(
            plot,
            hosts,
            placements,
            &["conduit.base/local@1".into()],
            options,
        )
        .unwrap();
    }
    let host = &hosts[0];
    let gears: BTreeSet<_> = plot
        .gears
        .iter()
        .filter(|gear| {
            let choice = &placements.by_gear[&gear.gear_id];
            let offer = host
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == choice.capability_id)
                .unwrap();
            conduit_ai::operation_owners::fixed_numeric::compact_pointwise_offer_layout(offer)
                .is_ok()
        })
        .map(|gear| gear.gear_id.clone())
        .collect();
    let compact = mode == 2;
    let offer = FusionRealizationOffer {
        fusion_id: "compact-proof/selected-dag".into(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        execution_profile_id: if compact {
            COMPACT_DAG_PROFILE
        } else {
            CANONICAL_DAG_PROFILE
        }
        .into(),
        implementation_id: if compact {
            COMPACT_DAG_IMPLEMENTATION
        } else {
            CANONICAL_DAG_IMPLEMENTATION
        }
        .into(),
        artifact_id: DAG_ARTIFACT.into(),
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        gear_ids: gears.iter().cloned().collect(),
        internal_cords: plot
            .connections
            .iter()
            .filter(|cord| {
                gears.contains(&cord.source_gear_id) && gears.contains(&cord.sink_gear_id)
            })
            .map(|cord| (cord.source_gear_id.clone(), cord.sink_gear_id.clone()))
            .collect(),
        preserves_typed_ports: true,
        preserves_atomic_pressure: true,
        preserves_cancellation: true,
        preserves_required_evidence: true,
    };
    let kinds = conduit_ai::fixed_numeric_catalog::fixed_numeric_contracts().unwrap();
    let choose = |offered: &FusionRealizationOffer,
                  semantic_kinds: &[Kind],
                  boundaries: &[FusionBoundary]| {
        plan_with_requested_fusions(
            plot,
            hosts,
            placements,
            &["conduit.base/local@1".into()],
            options,
            RequestedFusionInputs {
                offers: std::slice::from_ref(offered),
                semantic_kinds,
                boundaries,
            },
        )
    };
    if compact {
        let mut stale = offer.clone();
        stale.boot_id = "stale-host-boot".into();
        assert!(choose(&stale, &kinds, &[]).is_err());
        assert!(choose(&offer, &[], &[]).is_err());
        let (source, sink) = &offer.internal_cords[0];
        let boundary = FusionBoundary {
            source_gear_id: source.clone(),
            sink_gear_id: sink.clone(),
            requires_observation: true,
            requires_authority: false,
        };
        assert!(choose(&offer, &kinds, &[boundary]).is_err());
    }
    let optimized = choose(&offer, &kinds, &[]).unwrap();
    assert!(optimized.verify());
    if compact {
        // The general Plan schema allows a fusion to preserve a subset of its
        // internal edges. The complete Value DAG lowering must refuse that.
        let mut missing = optimized.plan.clone();
        missing.fragments[0].execution_fusions[0]
            .preserved_connections
            .remove(0);
        let identity = PlotIdentity {
            source_document_id: missing.source_document_id.clone(),
            checked_plot_id: missing.checked_plot_id.clone(),
            expanded_plot_id: missing.expanded_plot_id.clone(),
        };
        let missing = seal_plan_with_realization_backs_and_completion(
            identity,
            missing.completion_policy,
            missing.realization_backs,
            missing.fragments,
        );
        assert!(verify_plan(&missing));
        assert!(
            conduit_plan_lowering::lowering::lower_pure_value_fusion_dags(
                &missing,
                &missing.fragments[0].fragment_id
            )
            .is_err()
        );
    }
    optimized.plan
}
