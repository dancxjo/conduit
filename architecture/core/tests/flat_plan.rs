//! Core structural proof only: fixture IDs never create admitted native owners.
extern crate alloc;
use conduit_core::*;
mod api {
    pub use conduit_core::*;
}
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/fingerprint_refs.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn plan() -> Plan {
    let mut f = fixture::populated_fragment();
    f.realization_backs.clear();
    f.execution_regions.clear();
    f.execution_fusions.clear();
    f.states.clear();
    f.fore_ports.clear();
    f.shared_pools.clear();
    f.placements[0].pool_references.clear();
    let original = f.placements[0].clone();
    f.placements = (0..3)
        .map(|i| {
            let mut g = original.clone();
            g.placement_id = format!("node/{i}").into();
            g
        })
        .collect();
    let original = f.connections[0].clone();
    f.connections = (0..2)
        .map(|i| {
            let mut c = original.clone();
            c.connection_id = format!("cord/{i}").into();
            c.source_placement_id = f.placements[i].placement_id.clone();
            c.sink_placement_id = f.placements[i + 1].placement_id.clone();
            c.selected_line = None;
            c.admitted_lines.clear();
            c.resource = None;
            c
        })
        .collect();
    seal(f)
}
fn seal(f: PlanFragment) -> Plan {
    let identity = PlotIdentity {
        source_document_id: f.source_document_id.clone(),
        checked_plot_id: f.checked_plot_id.clone(),
        expanded_plot_id: f.expanded_plot_id.clone(),
    };
    seal_plan_with_completion(identity, PlanCompletionPolicy::Live, vec![f])
}
fn agrees(p: &Plan) -> bool {
    let rich = verify_plan(p);
    let flat = FlatPlanRef::from_plan(p).unwrap().verify().is_ok();
    assert_eq!(rich, flat);
    rich
}
#[test]
fn full_borrowed_flat_verification_is_zero_heap_and_matches_rich() {
    let p = plan();
    assert!(verify_plan(&p));
    let f = &p.fragments[0];
    let placements: Vec<_> = f.placements.iter().map(Into::into).collect();
    let connections: Vec<_> = f.connections.iter().map(Into::into).collect();
    let commitments: Vec<_> = f.plan_fragments.iter().map(Into::into).collect();
    let mut input = FlatPlanRef::from_plan(&p).unwrap();
    input.placements = PlannedGearRefs::Borrowed(&placements);
    input.tail.connections = PlannedConnectionRefs::Borrowed(&connections);
    input.root.commitments = FragmentCommitmentRefs::Borrowed(&commitments);
    input.fragment_commitments = FragmentCommitmentRefs::Borrowed(&commitments);
    let (verified, cost) = allocation_probe::observe(|| input.verify().unwrap());
    assert_eq!(
        (cost.allocations, cost.reallocations, cost.requested_bytes),
        (0, 0, 0)
    );
    assert_eq!(verified.view().plan_id, p.plan_id.as_str());
    assert_eq!(verified.view().placements.len(), 3);
    assert_eq!(compute_plan_fingerprint_ref(input.root), p.plan_id);
}
#[test]
fn identities_and_all_hashed_fields_are_not_alleged_authority() {
    let p = plan();
    assert!(agrees(&p));
    for field in 0..10 {
        let mut changed = p.clone();
        match field {
            0 => changed.plan_id = "forged".into(),
            1 => changed.source_document_id = "forged".into(),
            2 => changed.checked_plot_id = "forged".into(),
            3 => changed.expanded_plot_id = "forged".into(),
            4 => changed.fragments[0].fragment_id = "forged".into(),
            5 => changed.fragments[0].plan_id = "forged".into(),
            6 => changed.fragments[0].boot_id = "forged".into(),
            7 => changed.fragments[0].offer_generation.0 ^= 1,
            8 => changed.fragments[0].placements[0]
                .configuration
                .push(ConfigurationEntry {
                    key: "changed".into(),
                    value: ConfigurationValue::U64(1),
                }),
            _ => changed.fragments[0].connections[0].byte_capacity ^= 1,
        }
        assert!(!agrees(&changed), "field {field}");
    }
    let mut view = FlatPlanRef::from_plan(&p).unwrap();
    view.header.checked_plot_id = "contradictory-fragment";
    assert!(matches!(view.verify(), Err(FlatPlanRefusal::InvalidPlan)));
    let fabricated = FragmentCommitmentRef {
        host_id: p.fragments[0].host_id.as_str(),
        fragment_id: "forged",
    };
    let mut view = FlatPlanRef::from_plan(&p).unwrap();
    view.root.commitments = FragmentCommitmentRefs::Borrowed(core::slice::from_ref(&fabricated));
    assert!(matches!(view.verify(), Err(FlatPlanRefusal::InvalidPlan)));
}
#[test]
fn resealed_structural_forgeries_match_the_actual_rich_predicates() {
    for field in 0..8 {
        let mut f = plan().fragments.remove(0);
        match field {
            0 => f.placements[0].pool_references.push("absent-pool".into()),
            1 => f.connections.push(f.connections[0].clone()),
            2 => f.connections[0].source_placement_id = "absent".into(),
            3 => f.connections[0].sink_placement_id = "absent".into(),
            4 => f.placements.push(f.placements[0].clone()),
            5 => f.placements[0].host_id = "remote-without-line".into(),
            // Core does not validate port identities or placement Boot equality
            // here. Preserve that boundary rather than mint stronger authority.
            6 => f.connections[0].source_port_id = "not-a-Core-port-check".into(),
            _ => f.placements[0].boot_id = "not-a-Core-Boot-check".into(),
        }
        let p = seal(f);
        assert_eq!(agrees(&p), field >= 6, "field {field}");
    }
}
#[test]
fn unsupported_profiles_are_not_projected_away() {
    let rich = fixture::populated_fragment();
    for field in 0..6 {
        let mut f = plan().fragments.remove(0);
        match field {
            0 => f.states = rich.states.clone(),
            1 => f.shared_pools = rich.shared_pools.clone(),
            2 => f.fore_ports = rich.fore_ports.clone(),
            3 => f.execution_regions = rich.execution_regions.clone(),
            4 => f.execution_fusions = rich.execution_fusions.clone(),
            _ => f.connections[0].selected_line = rich.connections[0].selected_line.clone(),
        }
        let p = seal(f);
        assert!(matches!(
            FlatPlanRef::from_plan(&p).unwrap().verify(),
            Err(FlatPlanRefusal::UnsupportedProfile)
        ));
    }
}
#[test]
fn rich_streamed_configuration_callback_remains_authoritative() {
    let mut f = plan().fragments.remove(0);
    f.placements[0].configuration = vec![ConfigurationEntry {
        key: "exact".into(),
        value: ConfigurationValue::U64(u64::MAX),
    }];
    let p = seal(f);
    assert!(verify_plan(&p));
    let expected = p.fragments[0].placements[0].configuration.clone();
    let mut metadata = p.clone();
    for g in &mut metadata.fragments[0].placements {
        g.configuration.clear();
    }
    assert!(!verify_plan(&metadata));
    assert!(matches!(
        FlatPlanRef::from_plan(&metadata).unwrap().verify(),
        Err(FlatPlanRefusal::InvalidPlan)
    ));
    let verified = verify_plan_metadata_with_configurations(metadata.clone(), |g| {
        Ok::<_, ()>(
            if g.placement_id == p.fragments[0].placements[0].placement_id {
                expected.clone()
            } else {
                vec![]
            },
        )
    })
    .unwrap();
    assert_eq!(verified.plan_id(), &p.plan_id);
    assert!(matches!(
        verify_plan_metadata_with_configurations(metadata, |_| Ok::<_, ()>(vec![])),
        Err(PlanMetadataRefusal::InvalidPlan)
    ));
}

#[test]
fn streamed_callback_order_and_error_precedence_match_rich_verification() {
    let mut p = plan();
    p.plan_id = "incorrect-root".into();
    let mut calls = 0;
    assert!(matches!(
        verify_plan_metadata_with_configurations(p, |_| {
            calls += 1;
            Err::<Vec<ConfigurationEntry>, _>("must-not-call")
        }),
        Err(PlanMetadataRefusal::InvalidPlan)
    ));
    assert_eq!(calls, 0);
    let mut f = plan().fragments.remove(0);
    f.placements[0].pool_references.push("absent-pool".into());
    let p = seal(f);
    let mut calls = 0;
    assert!(matches!(
        verify_plan_metadata_with_configurations(p, |_| {
            calls += 1;
            Err::<Vec<ConfigurationEntry>, _>("configuration-first")
        }),
        Err(PlanMetadataRefusal::Configuration("configuration-first"))
    ));
    assert_eq!(calls, 1);
    let mut p = plan();
    p.fragments[0].plan_fragments.clear();
    let mut calls = 0;
    // Root fingerprint derives from actual fragments, not their claimed table;
    // a damaged table is checked only after the ordinary callback invocation.
    assert!(matches!(
        verify_plan_metadata_with_configurations(p, |_| {
            calls += 1;
            Err::<Vec<ConfigurationEntry>, _>("configuration-first")
        }),
        Err(PlanMetadataRefusal::Configuration("configuration-first"))
    ));
    assert_eq!(calls, 1);
}
