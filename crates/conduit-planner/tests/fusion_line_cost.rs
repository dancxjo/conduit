use std::collections::BTreeMap;

use conduit_core::{
    ArtifactId, BootId, CapabilityId, ExecutionProfileId, GearId, HostId, ImplementationId, LineId,
    OfferGeneration, SignId,
};
use conduit_planner::{
    plan_selected_optimization, select_fusion_candidate, CandidatePlacementDisposition,
    DataFlowObservation, FusionBoundary, FusionCandidate, FusionPlanningInputs,
    FusionPlanningObservation, FusionRealizationOffer, LocalCordObservation, LocalityCandidate,
    LocalityPlanningBasis, ObservationProvenance, PlacementChoice, PlacementChoices,
    PlanningOptions, RealizationWorkObservation, TransportObservation,
};

struct Fixture {
    form: conduit_form::CheckedForm,
    hosts: Vec<conduit_core::HostAdvertisement>,
    line: conduit_core::LineOffer,
}

fn fixture() -> Fixture {
    let form = conduit_form::parse(
        "form 0\n\nfusion {\n source: time/tick\n transform: flow/map\n analysis: flow/filter\n source.count = 10\n source.period-ms = 1\n source.tick -> transform.in\n transform > analysis\n}\n",
        &conduit_std_catalog::standard_profile_catalog(),
    )
    .expect("three-Gear fusion Form checks");
    let local = conduit_std_catalog::standard_host_advertisement(
        HostId::from("host/local"),
        BootId::from("boot/local-1"),
        OfferGeneration(1),
    );
    let remote = conduit_std_catalog::standard_host_advertisement(
        HostId::from("host/remote"),
        BootId::from("boot/remote-1"),
        OfferGeneration(1),
    );
    let mut line = conduit_signal::triple::exact_plan()
        .expect("exact Line fixture")
        .browser_line;
    line.line_id = LineId::from("line/local-to-remote");
    line.binding.source.host_id = local.host_id.clone();
    line.binding.source.boot_id = local.boot_id.clone();
    line.binding.sink.host_id = remote.host_id.clone();
    line.binding.sink.boot_id = remote.boot_id.clone();
    line.binding.limits.maximum_payload_bytes = 16;
    line.binding.limits.maximum_buffered_bytes = 64;
    line.binding.limits.maximum_frame_bytes = 32;
    line.availability.line_id = line.line_id.clone();
    line.availability.binding_id = line.binding.binding_id.clone();
    Fixture {
        form,
        hosts: vec![local, remote],
        line,
    }
}

fn provenance(id: &str) -> ObservationProvenance {
    ObservationProvenance {
        sign_id: SignId::from(id),
        source: "bounded scheduler comparison".into(),
        observed_at_ms: 900,
        valid_until_ms: 1_100,
    }
}

fn capability(fixture: &Fixture, gear: &str, host: usize) -> CapabilityId {
    let gear = fixture
        .form
        .gears
        .iter()
        .find(|item| item.gear_id.as_str() == gear)
        .unwrap();
    fixture.hosts[host]
        .capabilities
        .iter()
        .find(|offer| offer.checked_face() == gear.checked_face())
        .unwrap()
        .capability_id
        .clone()
}

fn placements(fixture: &Fixture, remote_split: bool) -> PlacementChoices {
    PlacementChoices {
        by_gear: ["source", "transform", "analysis"]
            .into_iter()
            .map(|gear| {
                let host = usize::from(remote_split && gear != "source");
                (
                    GearId::from(gear),
                    PlacementChoice {
                        host_id: fixture.hosts[host].host_id.clone(),
                        capability_id: capability(fixture, gear, host),
                    },
                )
            })
            .collect(),
    }
}

fn locality_candidate(fixture: &Fixture, id: &str, remote_split: bool) -> LocalityCandidate {
    LocalityCandidate {
        candidate_id: id.into(),
        placements: placements(fixture, remote_split),
        lines: if remote_split {
            BTreeMap::from([(
                (GearId::from("source"), GearId::from("transform")),
                fixture.line.line_id.clone(),
            )])
        } else {
            BTreeMap::new()
        },
    }
}

fn candidates(fixture: &Fixture) -> Vec<FusionCandidate> {
    vec![
        FusionCandidate {
            candidate_id: "all-local-unfused".into(),
            realization: locality_candidate(fixture, "all-local-unfused", false),
            fusion_ids: vec![],
        },
        FusionCandidate {
            candidate_id: "all-local-fused".into(),
            realization: locality_candidate(fixture, "all-local-fused", false),
            fusion_ids: vec!["fusion/local-chain".into()],
        },
        FusionCandidate {
            candidate_id: "remote-split".into(),
            realization: locality_candidate(fixture, "remote-split", true),
            fusion_ids: vec![],
        },
    ]
}

fn offer(fixture: &Fixture) -> FusionRealizationOffer {
    FusionRealizationOffer {
        fusion_id: "fusion/local-chain".into(),
        host_id: fixture.hosts[0].host_id.clone(),
        boot_id: fixture.hosts[0].boot_id.clone(),
        offer_generation: fixture.hosts[0].offer_generation,
        execution_profile_id: ExecutionProfileId::from("local/fused-chain-profile@1"),
        implementation_id: ImplementationId::from("local/fused-chain@1"),
        artifact_id: ArtifactId::from("local/fused-chain-artifact@1"),
        gear_ids: vec![
            GearId::from("source"),
            GearId::from("transform"),
            GearId::from("analysis"),
        ],
        internal_cords: vec![
            (GearId::from("source"), GearId::from("transform")),
            (GearId::from("transform"), GearId::from("analysis")),
        ],
        preserves_typed_ports: true,
        preserves_atomic_pressure: true,
        preserves_cancellation: true,
        preserves_required_evidence: true,
    }
}

fn basis(fixture: &Fixture, transport_per_kib: u64) -> LocalityPlanningBasis {
    let mut realization_work = Vec::new();
    for (gear, local, remote) in [
        ("source", 10, 10),
        ("transform", 100, 40),
        ("analysis", 100, 60),
    ] {
        for (host, work) in [(0, local), (1, remote)] {
            realization_work.push(RealizationWorkObservation {
                gear_id: GearId::from(gear),
                host_id: fixture.hosts[host].host_id.clone(),
                boot_id: fixture.hosts[host].boot_id.clone(),
                capability_id: capability(fixture, gear, host),
                work_units: work,
                provenance: provenance(&format!("work/{gear}/{host}")),
            });
        }
    }
    LocalityPlanningBasis {
        now_ms: 1_000,
        horizon_seconds: 10,
        remote_bytes_per_second_ceiling: None,
        data_flow: DataFlowObservation {
            source_gear_id: GearId::from("source"),
            items_per_second: 10_000,
            bytes_per_item: 1,
            provenance: provenance("flow/rate"),
        },
        reductions: vec![],
        realization_work,
        transports: vec![TransportObservation {
            line_id: fixture.line.line_id.clone(),
            source_host_id: fixture.hosts[0].host_id.clone(),
            sink_host_id: fixture.hosts[1].host_id.clone(),
            throughput_bytes_per_second: 20_000,
            setup_work_units: 20,
            bandwidth_work_units_per_kibibyte: transport_per_kib,
            serialization_work_units_per_kibibyte: 0,
            framing_work_units: 1,
            queueing_work_units: 1,
            latency_work_units: 5,
            jitter_work_units: 1,
            pressure_work_units: 5,
            cancellation_work_units: 1,
            loss_work_units: 1,
            provenance: provenance("line/cost"),
        }],
        local_cords: vec![
            LocalCordObservation {
                source_gear_id: GearId::from("source"),
                sink_gear_id: GearId::from("transform"),
                host_id: fixture.hosts[0].host_id.clone(),
                boot_id: fixture.hosts[0].boot_id.clone(),
                work_units: 10,
                provenance: provenance("cord/source-transform/local"),
            },
            LocalCordObservation {
                source_gear_id: GearId::from("transform"),
                sink_gear_id: GearId::from("analysis"),
                host_id: fixture.hosts[0].host_id.clone(),
                boot_id: fixture.hosts[0].boot_id.clone(),
                work_units: 10,
                provenance: provenance("cord/transform-analysis/local"),
            },
            LocalCordObservation {
                source_gear_id: GearId::from("transform"),
                sink_gear_id: GearId::from("analysis"),
                host_id: fixture.hosts[1].host_id.clone(),
                boot_id: fixture.hosts[1].boot_id.clone(),
                work_units: 10,
                provenance: provenance("cord/transform-analysis/remote-host"),
            },
        ],
        resources: fixture
            .hosts
            .iter()
            .flat_map(|host| {
                host.resources.iter().enumerate().map(move |(index, pool)| {
                    conduit_core::ResourceObservation {
                        host_id: host.host_id.clone(),
                        boot_id: host.boot_id.clone(),
                        offer_generation: host.offer_generation,
                        pool_id: pool.pool_id.clone(),
                        class_id: pool.class_id.clone(),
                        health: conduit_core::ResourceHealth::Ready,
                        unreserved_units: pool.capacity_units,
                        utilized_units: 0,
                        sign_id: SignId::from(format!(
                            "resource/{}/{index}",
                            host.host_id.as_str()
                        )),
                    }
                })
            })
            .collect(),
    }
}

fn fusion_observation(work: u64) -> FusionPlanningObservation {
    FusionPlanningObservation {
        fusion_id: "fusion/local-chain".into(),
        fused_work_units: work,
        provenance: provenance("fusion/work"),
    }
}

#[test]
fn safe_local_fusion_beats_unfused_and_tiny_remote_compute_gain() {
    let fixture = fixture();
    let selection = select_fusion_candidate(
        &fixture.form,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 2),
        FusionPlanningInputs {
            offers: &[offer(&fixture)],
            observations: &[fusion_observation(120)],
            boundaries: &[],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("three exact candidates compare");
    assert_eq!(selection.selected_candidate_id, "all-local-fused");
    assert_eq!(selection.considered[0].total_work_units, 230);
    assert_eq!(selection.considered[1].total_work_units, 120);
    assert_eq!(selection.considered[2].transported_bytes, 100_000);
    assert!(selection.considered[2].total_work_units > 210);
    assert!(selection.explain().contains("safely fused"));

    let optimized = plan_selected_optimization(
        &fixture.form,
        &fixture.hosts,
        &selection,
        &[conduit_core::ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("fusion explanation wraps the ordinary Plan");
    assert!(optimized.verify());
    assert_eq!(optimized.plan.checked_form_id, fixture.form.checked_form_id);
    assert_eq!(optimized.plan.fragments[0].placements.len(), 3);
    assert_eq!(optimized.plan.fragments[0].connections.len(), 2);
    assert_eq!(optimized.plan.fragments[0].execution_fusions.len(), 1);
    let planned_fusion = &optimized.plan.fragments[0].execution_fusions[0];
    assert!(optimized.plan.fragments[0]
        .placements
        .iter()
        .all(|placement| placement.implementation_id != planned_fusion.implementation_id));
    let lowered = conduit_runtime::lowering::lower_plan_fragment(&optimized.plan.fragments[0])
        .expect("selected fusion lowers through the ordinary numeric graph");
    assert_eq!(lowered.nodes.len(), 3);
    assert_eq!(lowered.cords.len(), 2);
    assert_eq!(lowered.fusions.len(), 1);
    assert_eq!(lowered.fusions[0].nodes.len(), 3);
    assert_eq!(lowered.fusions[0].cords.len(), 2);

    let unfused_selection = select_fusion_candidate(
        &fixture.form,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 2),
        FusionPlanningInputs {
            offers: &[offer(&fixture)],
            observations: &[fusion_observation(120)],
            boundaries: &[FusionBoundary {
                source_gear_id: GearId::from("transform"),
                sink_gear_id: GearId::from("analysis"),
                requires_observation: true,
                requires_authority: false,
            }],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("required observation retains the ordinary local realization");
    let unfused = plan_selected_optimization(
        &fixture.form,
        &fixture.hosts,
        &unfused_selection,
        &[conduit_core::ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("unfused ordinary Plan");
    assert_ne!(optimized.plan.plan_id, unfused.plan.plan_id);
    assert_eq!(
        optimized.plan.fragments[0].cancellation_policy,
        unfused.plan.fragments[0].cancellation_policy
    );
    assert_eq!(
        optimized.plan.fragments[0].connections,
        unfused.plan.fragments[0].connections
    );

    let mut mutated = optimized.plan.clone();
    mutated.fragments[0].execution_fusions[0]
        .preserved_connections
        .clear();
    assert!(!conduit_core::verify_plan(&mutated));
}

#[test]
fn remote_split_wins_only_when_advantage_exceeds_line_cost() {
    let fixture = fixture();
    let expensive_fusion = fusion_observation(3_000);
    let selection = select_fusion_candidate(
        &fixture.form,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 0),
        FusionPlanningInputs {
            offers: &[offer(&fixture)],
            observations: &[expensive_fusion],
            boundaries: &[],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("substantial remote advantage wins despite finite setup cost");
    assert_eq!(selection.selected_candidate_id, "remote-split");
    assert_eq!(selection.considered[2].compute_work_units, 110);
    assert_eq!(selection.considered[2].transport_work_units, 45);
}

#[test]
fn observation_authority_and_semantic_preservation_can_forbid_fusion() {
    let fixture = fixture();
    for boundary in [
        FusionBoundary {
            source_gear_id: GearId::from("transform"),
            sink_gear_id: GearId::from("analysis"),
            requires_observation: true,
            requires_authority: false,
        },
        FusionBoundary {
            source_gear_id: GearId::from("transform"),
            sink_gear_id: GearId::from("analysis"),
            requires_observation: false,
            requires_authority: true,
        },
    ] {
        let selection = select_fusion_candidate(
            &fixture.form,
            &fixture.hosts,
            &candidates(&fixture),
            &basis(&fixture, 2),
            FusionPlanningInputs {
                offers: &[offer(&fixture)],
                observations: &[fusion_observation(120)],
                boundaries: &[boundary],
                line_offers: std::slice::from_ref(&fixture.line),
            },
        )
        .expect("unfused candidate remains available");
        assert_eq!(selection.selected_candidate_id, "all-local-unfused");
        assert!(matches!(
            selection.considered[1].disposition,
            CandidatePlacementDisposition::Rejected(_)
        ));
    }

    let mut unsafe_offer = offer(&fixture);
    unsafe_offer.preserves_cancellation = false;
    let selection = select_fusion_candidate(
        &fixture.form,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 2),
        FusionPlanningInputs {
            offers: &[unsafe_offer],
            observations: &[fusion_observation(1)],
            boundaries: &[],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("ordinary candidates survive an unsafe fusion offer");
    assert_ne!(selection.selected_candidate_id, "all-local-fused");

    let mut stale_offer = offer(&fixture);
    stale_offer.offer_generation = OfferGeneration(2);
    let selection = select_fusion_candidate(
        &fixture.form,
        &fixture.hosts,
        &candidates(&fixture),
        &basis(&fixture, 2),
        FusionPlanningInputs {
            offers: &[stale_offer],
            observations: &[fusion_observation(1)],
            boundaries: &[],
            line_offers: std::slice::from_ref(&fixture.line),
        },
    )
    .expect("ordinary candidates survive a stale Host fusion offer");
    assert_ne!(selection.selected_candidate_id, "all-local-fused");
    assert!(matches!(
        selection.considered[1].disposition,
        CandidatePlacementDisposition::Rejected(_)
    ));
}
