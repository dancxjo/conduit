#![allow(dead_code)]
use std::collections::BTreeMap;

use conduit_core::{
    ArtifactId, BootId, CapabilityId, ExecutionProfileId, GearId, HostId, ImplementationId, LineId,
    SignId,
};
use conduit_planner::{
    DataFlowObservation, FusionCandidate, FusionPlanningObservation, FusionRealizationOffer,
    LocalCordObservation, LocalityCandidate, LocalityPlanningBasis, ObservationProvenance,
    PlacementChoice, PlacementChoices, RealizationWorkObservation, TransportObservation,
};

use super::common;

pub(crate) struct Fixture {
    pub(crate) plot: conduit_plot::CheckedPlot,
    pub(crate) semantic_kinds: Vec<conduit_core::Kind>,
    pub(crate) hosts: Vec<conduit_core::HostAdvertisement>,
    pub(crate) line: conduit_core::LineOffer,
}

pub(crate) fn fixture() -> Fixture {
    let profile = conduit_semantic_catalog::standard_profile_catalog();
    let plot = conduit_plot::parse(
        "plot fusion (\n input: Scalar >> output: Scalar\n) {\n source: math/clamp\n transform: math/scale\n analysis: math/deadband\n input >> source >> transform >> analysis >> output\n}\n",
        &profile,
    )
    .expect("three-Gear fusion Plot checks");
    let semantic_kinds = vec![
        conduit_semantic_catalog::math_clamp_semantic_contract(),
        conduit_semantic_catalog::math_scale_semantic_contract(),
        conduit_semantic_catalog::math_deadband_semantic_contract(),
    ];
    let local =
        common::standard_planning_fixture(HostId::from("host/local"), BootId::from("boot/local-1"));
    let remote = common::standard_planning_fixture(
        HostId::from("host/remote"),
        BootId::from("boot/remote-1"),
    );
    let mut line = conduit_signal_conformance::triple::exact_plan()
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
        plot,
        semantic_kinds,
        hosts: vec![local, remote],
        line,
    }
}

pub(crate) fn provenance(id: &str) -> ObservationProvenance {
    ObservationProvenance {
        sign_id: SignId::from(id),
        source: "bounded scheduler comparison".into(),
        observed_at_ms: 900,
        valid_until_ms: 1_100,
    }
}

pub(crate) fn capability(fixture: &Fixture, gear: &str, host: usize) -> CapabilityId {
    let gear = format!("fusion/{gear}");
    let gear = fixture
        .plot
        .gears
        .iter()
        .find(|item| item.gear_id.as_str() == gear)
        .unwrap();
    fixture.hosts[host]
        .capabilities
        .iter()
        .find(|offer| {
            offer.kind_id == gear.kind_id
                && offer.kind_contract_revision == gear.kind_contract_revision
                && offer.checked_front() == gear.checked_front()
        })
        .unwrap()
        .capability_id
        .clone()
}

pub(crate) fn placements(fixture: &Fixture, remote_split: bool) -> PlacementChoices {
    PlacementChoices {
        by_gear: ["source", "transform", "analysis"]
            .into_iter()
            .map(|gear| {
                let host = usize::from(remote_split && gear != "source");
                (
                    GearId::from(format!("fusion/{gear}")),
                    PlacementChoice {
                        host_id: fixture.hosts[host].host_id.clone(),
                        capability_id: capability(fixture, gear, host),
                    },
                )
            })
            .collect(),
    }
}

pub(crate) fn locality_candidate(
    fixture: &Fixture,
    id: &str,
    remote_split: bool,
) -> LocalityCandidate {
    LocalityCandidate {
        candidate_id: id.into(),
        placements: placements(fixture, remote_split),
        lines: if remote_split {
            BTreeMap::from([(
                (
                    GearId::from("fusion/source"),
                    GearId::from("fusion/transform"),
                ),
                fixture.line.line_id.clone(),
            )])
        } else {
            BTreeMap::new()
        },
    }
}

pub(crate) fn candidates(fixture: &Fixture) -> Vec<FusionCandidate> {
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

pub(crate) fn offer(fixture: &Fixture) -> FusionRealizationOffer {
    FusionRealizationOffer {
        fusion_id: "fusion/local-chain".into(),
        host_id: fixture.hosts[0].host_id.clone(),
        boot_id: fixture.hosts[0].boot_id.clone(),
        offer_generation: fixture.hosts[0].offer_generation,
        execution_profile_id: ExecutionProfileId::from("local/fused-chain-profile@1"),
        implementation_id: ImplementationId::from("local/fused-chain@1"),
        artifact_id: ArtifactId::from("local/fused-chain-artifact@1"),
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        gear_ids: vec![
            GearId::from("fusion/source"),
            GearId::from("fusion/transform"),
            GearId::from("fusion/analysis"),
        ],
        internal_cords: vec![
            (
                GearId::from("fusion/source"),
                GearId::from("fusion/transform"),
            ),
            (
                GearId::from("fusion/transform"),
                GearId::from("fusion/analysis"),
            ),
        ],
        preserves_typed_ports: true,
        preserves_atomic_pressure: true,
        preserves_cancellation: true,
        preserves_required_evidence: true,
    }
}

pub(crate) fn basis(fixture: &Fixture, transport_per_kib: u64) -> LocalityPlanningBasis {
    let mut realization_work = Vec::new();
    for (gear, local, remote) in [
        ("source", 10, 10),
        ("transform", 100, 40),
        ("analysis", 100, 60),
    ] {
        for (host, work) in [(0, local), (1, remote)] {
            realization_work.push(RealizationWorkObservation {
                gear_id: GearId::from(format!("fusion/{gear}")),
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
            source_gear_id: GearId::from("fusion/source"),
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
                source_gear_id: GearId::from("fusion/source"),
                sink_gear_id: GearId::from("fusion/transform"),
                host_id: fixture.hosts[0].host_id.clone(),
                boot_id: fixture.hosts[0].boot_id.clone(),
                work_units: 10,
                provenance: provenance("cord/source-transform/local"),
            },
            LocalCordObservation {
                source_gear_id: GearId::from("fusion/transform"),
                sink_gear_id: GearId::from("fusion/analysis"),
                host_id: fixture.hosts[0].host_id.clone(),
                boot_id: fixture.hosts[0].boot_id.clone(),
                work_units: 10,
                provenance: provenance("cord/transform-analysis/local"),
            },
            LocalCordObservation {
                source_gear_id: GearId::from("fusion/transform"),
                sink_gear_id: GearId::from("fusion/analysis"),
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

pub(crate) fn fusion_observation(work: u64) -> FusionPlanningObservation {
    FusionPlanningObservation {
        fusion_id: "fusion/local-chain".into(),
        fused_work_units: work,
        provenance: provenance("fusion/work"),
    }
}
