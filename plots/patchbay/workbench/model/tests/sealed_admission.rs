//! Actual installed numerical bounds must reach read-only Plan inspection.
use conduit_core::*;
use conduit_observatory::*;
use conduit_planner::*;
use conduit_plot::*;
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use std::collections::BTreeMap;

#[test]
fn installed_near_scratch_and_work_bounds_survive_patchbay_projection() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source = "plot project (\n >> value: Quantity <= 1800B\n result: NearQuantity <= 16384B >>\n) {\n boundary: math/near(limit=0.0001m)\n value >> boundary.value\n boundary.result >> result\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "project", &profile).unwrap();
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "host/admission-inspection".into(),
            boot_id: "boot/admission-inspection".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    host.prepare_numerical_realizations(&authored.expanded)
        .unwrap();
    let advertisement = host.advertisement().clone();
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
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
                byte_capacity: 16384,
            },
        )
    })
    .collect();
    let plan = plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 16384,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    let original = plan.fragments[0].placements[0].clone();
    assert!(!original.realization_properties.is_empty());
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: vec![HostReport {
            capabilities: advertisement
                .capabilities
                .iter()
                .map(|offer| CapabilityStatusReport {
                    capability_id: offer.capability_id.clone(),
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
        plans: vec![plan],
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
    let report = patchbay.current_report().unwrap();
    let admission = report.placements[0].admission.as_ref().unwrap();
    assert_eq!(
        admission.realization_properties,
        original.realization_properties
    );
    assert_eq!(admission.limits, original.limits);
    let text = patchbay.document(None).unwrap().lines().join("\n");
    assert!(text.contains("sealed-admission"));
    for property in &original.realization_properties {
        assert!(text.contains(property.profile().as_str()));
    }
    let restored: ObservatoryReport =
        serde_json::from_str(&serde_json::to_string(report).unwrap()).unwrap();
    assert_eq!(*report, restored);
}
