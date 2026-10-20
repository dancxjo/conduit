//! The frozen 32-Gear measurement source remains fully inspectable in Patchbay.
use conduit_core::*;
use conduit_observatory::*;
use conduit_planner::*;
use conduit_plot::*;
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use std::collections::BTreeMap;
#[path = "support/fusion_runtime.rs"]
mod fusion_runtime;

#[test]
fn all_thirty_two_logical_gears_and_cords_survive_physical_inspection() {
    let source =
        include_str!("../../../../../proof/performance/runtime/fixtures/pure-chain-32.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let plot =
        expand_canonical_plot_for_authoring(&checked, "baseline", &ProfileCatalog::new()).unwrap();
    let host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "host/patchbay-fusion".into(),
            boot_id: "boot/patchbay-fusion".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    let mut advertisement = host.advertisement().clone();
    for gear in &plot.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("checked program absent")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer = conduit_std_offers::pure_expression_local_flow_offer(&program).unwrap();
        if !advertisement
            .capabilities
            .iter()
            .any(|c| c.capability_id == offer.capability_id)
        {
            advertisement.capabilities.push(offer);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(&plot.expanded, &hosts).unwrap();
    let (offer, kinds) =
        conduit_std_host::pure_chain_fusion::pure_chain_fusion_offer(&plot, &advertisement)
            .unwrap();
    let provenance = |id: &str| ObservationProvenance {
        sign_id: format!("sign/patchbay-fusion/{id}").into(),
        source: "inspection fixture work assumptions; no timing claim".into(),
        observed_at_ms: 0,
        valid_until_ms: 0,
    };
    let basis = LocalityPlanningBasis {
        now_ms: 0,
        horizon_seconds: 1,
        remote_bytes_per_second_ceiling: Some(0),
        data_flow: DataFlowObservation {
            source_gear_id: offer.gear_ids[0].clone(),
            items_per_second: 64,
            bytes_per_item: 8,
            provenance: provenance("flow"),
        },
        reductions: vec![],
        transports: vec![],
        resources: vec![],
        realization_work: plot
            .expanded
            .gears
            .iter()
            .map(|gear| RealizationWorkObservation {
                gear_id: gear.gear_id.clone(),
                host_id: advertisement.host_id.clone(),
                boot_id: advertisement.boot_id.clone(),
                capability_id: placements.by_gear[&gear.gear_id].capability_id.clone(),
                work_units: 2,
                provenance: provenance(gear.gear_id.as_str()),
            })
            .collect(),
        local_cords: plot
            .expanded
            .connections
            .iter()
            .map(|cord| LocalCordObservation {
                source_gear_id: cord.source_gear_id.clone(),
                sink_gear_id: cord.sink_gear_id.clone(),
                host_id: advertisement.host_id.clone(),
                boot_id: advertisement.boot_id.clone(),
                work_units: 0,
                provenance: provenance(&format!("cord/{}", cord.source_gear_id.as_str())),
            })
            .collect(),
    };
    let mut candidates: Vec<_> = [false, true]
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
    candidates.push(FusionCandidate {
        candidate_id: "unavailable-fusion".into(),
        realization: LocalityCandidate {
            candidate_id: "unavailable-fusion".into(),
            placements: placements.clone(),
            lines: BTreeMap::new(),
        },
        fusion_ids: vec!["not-offered".into()],
    });
    let selection = select_expanded_fusion_candidate(
        &plot.expanded,
        &hosts,
        &candidates,
        &basis,
        FusionPlanningInputs {
            semantic_kinds: &kinds,
            offers: core::slice::from_ref(&offer),
            observations: &[FusionPlanningObservation {
                fusion_id: offer.fusion_id.clone(),
                fused_work_units: 33,
                provenance: provenance("fused"),
            }],
            boundaries: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    assert_eq!(selection.selected_candidate_id, "fused");
    let boundary_limits = [
        (PortDirection::Input, "input"),
        (PortDirection::Output, "output"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 8,
            },
        )
    })
    .collect();
    let optimized = plan_expanded_selected_optimization(
        &plot,
        &hosts,
        &selection,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .unwrap();
    let unfused = fusion_runtime::assert_actual_failure_custody(&optimized.plan, &advertisement);
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: vec![HostReport {
            capabilities: advertisement
                .capabilities
                .iter()
                .map(|c| CapabilityStatusReport {
                    capability_id: c.capability_id.clone(),
                    freshness: OfferFreshness::Fresh,
                    support: CapabilitySupport::Supported,
                    availability: CapabilityAvailability::Available,
                })
                .collect(),
            advertisement,
            devices: vec![],
            state: OperationalState::Available,
        }],
        bases: vec![],
        lines: vec![],
        plans: vec![optimized.plan.clone()],
        plays: vec![],
        observations: vec![],
        historical_observations: vec![],
        sealed_boot_provenance: vec![],
        retention: RetentionReport {
            item_capacity: 1,
            retained_items: 0,
            dropped_items: 0,
        },
    };
    let mut patchbay = conduit_patchbay_workbench::PatchbayTopology::new(1).unwrap();
    patchbay.ingest(&snapshot).unwrap();
    let explanation = patchbay
        .document_with_fusion_selection(&optimized, &selection)
        .unwrap();
    let explanation = explanation.lines().join("\n");
    assert!(explanation.contains("FUSION SELECTION"));
    assert!(explanation.contains("CANDIDATE fused disposition=Selected"));
    assert!(explanation.contains("CANDIDATE unfused disposition=Admitted"));
    assert!(explanation.contains("CANDIDATE unavailable-fusion disposition=Rejected"));
    assert!(explanation.contains("sign/patchbay-fusion/fused"));
    let mut wrong = selection.clone();
    wrong.checked_plot_id = "foreign-checked".into();
    assert!(patchbay
        .document_with_fusion_selection(&optimized, &wrong)
        .is_err());
    let mut wrong = selection.clone();
    wrong
        .considered
        .iter_mut()
        .find(|c| c.candidate_id == "fused")
        .unwrap()
        .total_work_units += 1;
    assert!(patchbay
        .document_with_fusion_selection(&optimized, &wrong)
        .is_err());
    let mut wrong = selection.clone();
    wrong.locality_basis.realization_work[0].boot_id = "foreign-boot".into();
    assert!(patchbay
        .document_with_fusion_selection(&optimized, &wrong)
        .is_err());
    let mut oversized = selection.clone();
    oversized
        .considered
        .iter_mut()
        .find(|c| c.candidate_id == "unavailable-fusion")
        .unwrap()
        .disposition = CandidatePlacementDisposition::Rejected("x".repeat(70 * 1024));
    assert_eq!(
        patchbay
            .document_with_fusion_selection(&optimized, &oversized)
            .unwrap_err(),
        conduit_patchbay_workbench::TopologyViewError::FaceTooLarge
    );
    let report = patchbay.current_report().unwrap();
    assert_eq!(report.execution_fusions.len(), 1);
    assert_eq!(report.placements.len(), 32);
    assert_eq!(report.connections.len(), 31);
    let fragment = &snapshot.plans[0].fragments[0];
    let admission = report.fragments[0].admission.as_ref().unwrap();
    assert_eq!(admission.sign_storage_budget, fragment.sign_storage_budget);
    assert_eq!(admission.expected_sign, fragment.expected_sign);
    assert_eq!(admission.shared_pools, fragment.shared_pools);
    assert_eq!(admission.cancellation_policy, fragment.cancellation_policy);
    assert_eq!(admission.terminal_policy, fragment.terminal_policy);
    for row in &report.placements {
        let logical = row
            .logical_gear
            .as_ref()
            .expect("logical Gear identity missing");
        let original = snapshot.plans[0].fragments[0]
            .placements
            .iter()
            .find(|p| p.placement_id == row.placement_id)
            .unwrap();
        let admission = row.admission.as_ref().unwrap();
        assert_eq!(admission.limits, original.limits);
        assert_eq!(
            admission.realization_characteristics,
            original.realization_characteristics
        );
        assert_eq!(
            admission.realization_properties,
            original.realization_properties
        );
        assert_eq!(admission.pool_references, original.pool_references);
        assert_eq!(
            admission.terminal_transductions,
            original.terminal_transductions
        );
        assert_eq!(logical.gear_id, original.gear_id);
        assert_eq!(logical.inputs, original.inputs);
        assert_eq!(logical.outputs, original.outputs);
        assert_eq!(logical.source_span, original.source_span);
    }
    for row in &report.connections {
        let logical = row
            .logical_cord
            .as_ref()
            .expect("logical Cord contract missing");
        let original = snapshot.plans[0].fragments[0]
            .connections
            .iter()
            .find(|c| c.connection_id == row.connection_id)
            .unwrap();
        assert_eq!(logical.source_port_id, original.source_port_id);
        assert_eq!(logical.sink_port_id, original.sink_port_id);
        assert_eq!(logical.track, original.track);
        assert_eq!(logical.temporal, original.temporal);
        assert_eq!(logical.pressure_policy, original.pressure_policy);
    }
    let text = patchbay.document(None).unwrap().lines().join("\n");
    assert!(text.contains("FUSION"));
    for placement in &snapshot.plans[0].fragments[0].placements {
        assert!(
            text.contains(placement.gear_id.as_str()),
            "missing {}",
            placement.gear_id.as_str()
        );
        assert!(placement.source_span.is_some());
    }
    for cord in &report.connections {
        assert!(text.contains(cord.connection_id.as_str()));
    }
    let retained: ObservatoryReport =
        serde_json::from_str(&serde_json::to_string(report).unwrap()).unwrap();
    assert_eq!(retained, *report);
    assert!(text.contains("sealed-admission"));
    assert!(text.contains("sign_storage_budget"));
    // Historical reports without admission retain an explicit unknown value.
    let mut historical = serde_json::to_value(report).unwrap();
    for row in historical["placements"].as_array_mut().unwrap() {
        row.as_object_mut().unwrap().remove("admission");
    }
    for row in historical["fragments"].as_array_mut().unwrap() {
        row.as_object_mut().unwrap().remove("admission");
    }
    let historical: ObservatoryReport = serde_json::from_value(historical).unwrap();
    assert!(historical
        .placements
        .iter()
        .all(|row| row.admission.is_none()));
    assert!(historical
        .fragments
        .iter()
        .all(|row| row.admission.is_none()));
    let logical_gears: Vec<_> = report
        .placements
        .iter()
        .map(|row| row.logical_gear.clone())
        .collect();
    let logical_cords: Vec<_> = report
        .connections
        .iter()
        .map(|row| row.logical_cord.clone())
        .collect();
    let mut ordinary = snapshot.clone();
    ordinary.plans = vec![unfused];
    patchbay.ingest(&ordinary).unwrap();
    let report = patchbay.current_report().unwrap();
    assert!(report.execution_fusions.is_empty());
    assert_eq!(
        logical_gears,
        report
            .placements
            .iter()
            .map(|row| row.logical_gear.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        logical_cords,
        report
            .connections
            .iter()
            .map(|row| row.logical_cord.clone())
            .collect::<Vec<_>>()
    );
}
