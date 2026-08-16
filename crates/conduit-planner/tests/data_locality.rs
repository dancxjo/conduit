use std::collections::BTreeMap;

use conduit_core::{BootId, CapabilityId, GearId, HostId, LineId, OfferGeneration, SignId};
use conduit_planner::{
    select_data_locality_candidate, CandidatePlacementDisposition, DataFlowObservation,
    LocalCordObservation, LocalityCandidate, LocalityPlanningBasis, ObservationProvenance,
    PlacementChoice, PlacementChoices, RealizationWorkObservation, ReductionObservation,
    TransportObservation,
};

fn fixture() -> (
    conduit_form::CheckedForm,
    Vec<conduit_core::HostAdvertisement>,
    conduit_core::LineOffer,
) {
    let form = conduit_form::parse(
        "form 0\n\nlocality {\n source: time/tick\n reduction: flow/filter\n analysis: flow/map\n source.count = 10\n source.period-ms = 1\n source.tick -> reduction.in\n reduction > analysis\n}\n",
        &conduit_std_catalog::standard_profile_catalog(),
    ).expect("canonical locality Form checks");
    let source = conduit_std_catalog::standard_host_advertisement(
        HostId::from("host/constrained"),
        BootId::from("boot/constrained-1"),
        OfferGeneration(1),
    );
    let remote = conduit_std_catalog::standard_host_advertisement(
        HostId::from("host/analysis"),
        BootId::from("boot/analysis-1"),
        OfferGeneration(1),
    );
    let mut line = conduit_signal::triple::exact_plan()
        .expect("Line fixture")
        .browser_line;
    line.line_id = LineId::from("line/constrained-to-analysis");
    line.binding.source.host_id = source.host_id.clone();
    line.binding.source.boot_id = source.boot_id.clone();
    line.binding.sink.host_id = remote.host_id.clone();
    line.binding.sink.boot_id = remote.boot_id.clone();
    line.availability.line_id = line.line_id.clone();
    line.availability.binding_id = line.binding.binding_id.clone();
    line.binding.limits.maximum_payload_bytes = 2_000;
    line.binding.limits.maximum_frame_bytes = 2_100;
    line.binding.limits.maximum_buffered_bytes = 32_000;
    (form, vec![source, remote], line)
}

fn provenance(id: &str) -> ObservationProvenance {
    ObservationProvenance {
        sign_id: SignId::from(id),
        source: "bounded benchmark fixture".into(),
        observed_at_ms: 900,
        valid_until_ms: 1_100,
    }
}

fn capability(
    form: &conduit_form::CheckedForm,
    hosts: &[conduit_core::HostAdvertisement],
    gear: &str,
    host: usize,
) -> CapabilityId {
    let gear = form
        .gears
        .iter()
        .find(|item| item.gear_id.as_str() == gear)
        .unwrap();
    hosts[host]
        .capabilities
        .iter()
        .find(|offer| offer.checked_face() == gear.checked_face())
        .unwrap()
        .capability_id
        .clone()
}

fn choice(
    form: &conduit_form::CheckedForm,
    hosts: &[conduit_core::HostAdvertisement],
    gear: &str,
    host: usize,
) -> (GearId, PlacementChoice) {
    (
        GearId::from(gear),
        PlacementChoice {
            host_id: hosts[host].host_id.clone(),
            capability_id: capability(form, hosts, gear, host),
        },
    )
}

fn candidate(
    form: &conduit_form::CheckedForm,
    hosts: &[conduit_core::HostAdvertisement],
    id: &str,
    reduction_host: usize,
) -> LocalityCandidate {
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            choice(form, hosts, "source", 0),
            choice(form, hosts, "reduction", reduction_host),
            choice(form, hosts, "analysis", 1),
        ]),
    };
    let crossing = if reduction_host == 0 {
        (GearId::from("reduction"), GearId::from("analysis"))
    } else {
        (GearId::from("source"), GearId::from("reduction"))
    };
    LocalityCandidate {
        candidate_id: id.into(),
        placements,
        lines: BTreeMap::from([(crossing, LineId::from("line/constrained-to-analysis"))]),
    }
}

fn basis(
    form: &conduit_form::CheckedForm,
    hosts: &[conduit_core::HostAdvertisement],
    local_reduction_work: u64,
) -> LocalityPlanningBasis {
    let mut work = Vec::new();
    for (gear, host, units) in [
        ("source", 0, 10),
        ("reduction", 0, local_reduction_work),
        ("reduction", 1, 50),
        ("analysis", 1, 100),
    ] {
        work.push(RealizationWorkObservation {
            gear_id: GearId::from(gear),
            host_id: hosts[host].host_id.clone(),
            boot_id: hosts[host].boot_id.clone(),
            capability_id: capability(form, hosts, gear, host),
            work_units: units,
            provenance: provenance(&format!("work/{gear}/{host}")),
        });
    }
    LocalityPlanningBasis {
        now_ms: 1_000,
        horizon_seconds: 10,
        remote_bytes_per_second_ceiling: None,
        data_flow: DataFlowObservation {
            source_gear_id: GearId::from("source"),
            items_per_second: 100_000,
            bytes_per_item: 1,
            provenance: provenance("flow/high-rate"),
        },
        reductions: vec![ReductionObservation {
            gear_id: GearId::from("reduction"),
            output_items_numerator: 1,
            input_items_denominator: 10,
            output_bytes_numerator: 1,
            input_bytes_denominator: 10,
            provenance: provenance("reduction/profile"),
        }],
        realization_work: work,
        transports: vec![TransportObservation {
            line_id: LineId::from("line/constrained-to-analysis"),
            source_host_id: hosts[0].host_id.clone(),
            sink_host_id: hosts[1].host_id.clone(),
            throughput_bytes_per_second: 200_000,
            setup_work_units: 20,
            bandwidth_work_units_per_kibibyte: 1,
            serialization_work_units_per_kibibyte: 1,
            framing_work_units: 1,
            queueing_work_units: 1,
            latency_work_units: 5,
            jitter_work_units: 1,
            pressure_work_units: 5,
            cancellation_work_units: 1,
            loss_work_units: 1,
            provenance: provenance("line/current"),
        }],
        local_cords: vec![
            LocalCordObservation {
                source_gear_id: GearId::from("source"),
                sink_gear_id: GearId::from("reduction"),
                host_id: hosts[0].host_id.clone(),
                boot_id: hosts[0].boot_id.clone(),
                work_units: 5,
                provenance: provenance("cord/source-reduction/local"),
            },
            LocalCordObservation {
                source_gear_id: GearId::from("reduction"),
                sink_gear_id: GearId::from("analysis"),
                host_id: hosts[1].host_id.clone(),
                boot_id: hosts[1].boot_id.clone(),
                work_units: 5,
                provenance: provenance("cord/reduction-analysis/remote-host"),
            },
        ],
        resources: hosts
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

#[test]
fn high_rate_flow_moves_compatible_reduction_to_the_source() {
    let (form, hosts, line) = fixture();
    let candidates = [
        candidate(&form, &hosts, "reduce-near-source", 0),
        candidate(&form, &hosts, "ship-raw-then-reduce", 1),
    ];
    let selection = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &basis(&form, &hosts, 80),
        std::slice::from_ref(&line),
    )
    .expect("fresh bounded evidence selects");
    assert_eq!(
        selection.selected.candidate_id, "reduce-near-source",
        "{:#?}",
        selection.considered
    );
    assert_eq!(selection.checked_form_id, form.checked_form_id);
    let local = &selection.considered[0];
    let remote = &selection.considered[1];
    assert_eq!(local.transported_bytes, 100_000);
    assert_eq!(remote.transported_bytes, 1_000_000);
    assert!(local.total_work_units < remote.total_work_units);
    assert!(matches!(
        local.disposition,
        CandidatePlacementDisposition::Selected
    ));
    assert!(local
        .supporting_sign_ids
        .iter()
        .any(|id| id.as_str() == "line/current"));
    assert!(selection.explain().contains("would need at least"));
    assert_eq!(
        selection
            .planning_basis
            .data_flow
            .provenance
            .sign_id
            .as_str(),
        "flow/high-rate"
    );

    let line_candidates = selection
        .selected
        .lines
        .iter()
        .map(|(cord, line)| (cord.clone(), vec![line.clone()]))
        .collect();
    let plan = conduit_planner::plan_with_options(
        &form,
        &hosts,
        &selection.selected.placements,
        &[
            conduit_core::ConnectionBase::Local,
            conduit_core::ConnectionBase::WebSocket,
        ],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[line],
        },
    )
    .expect("winner enters the ordinary immutable Plan path");
    assert_eq!(plan.checked_form_id, form.checked_form_id);
    assert!(conduit_core::verify_plan(&plan));
}

#[test]
fn remote_reduction_wins_when_local_work_is_genuinely_too_expensive() {
    let (form, hosts, line) = fixture();
    let candidates = [
        candidate(&form, &hosts, "reduce-near-source", 0),
        candidate(&form, &hosts, "ship-raw-then-reduce", 1),
    ];
    let selection = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &basis(&form, &hosts, 10_000),
        &[line],
    )
    .expect("remote total cost can win");
    assert_eq!(selection.selected.candidate_id, "ship-raw-then-reduce");
    assert_eq!(
        selection.selected.placements.by_gear[&GearId::from("reduction")].host_id,
        hosts[1].host_id
    );
}

#[test]
fn stale_or_inadequate_observations_fail_closed_without_host_name_rules() {
    let (form, hosts, line) = fixture();
    let candidates = [
        candidate(&form, &hosts, "local", 0),
        candidate(&form, &hosts, "remote", 1),
    ];
    let mut stale = basis(&form, &hosts, 80);
    stale.transports[0].provenance.valid_until_ms = 999;
    assert!(matches!(
        select_data_locality_candidate(
            &form,
            &hosts,
            &candidates,
            &stale,
            std::slice::from_ref(&line),
        ),
        Err(conduit_planner::PlannerError::InvalidPlanningObservation(_))
    ));
    let mut inadequate = basis(&form, &hosts, 80);
    inadequate.transports[0].throughput_bytes_per_second = 50_000;
    let selected = select_data_locality_candidate(&form, &hosts, &candidates, &inadequate, &[line])
        .expect("local reduced traffic still fits");
    assert_eq!(selected.selected.candidate_id, "local");
    assert!(matches!(
        selected.considered[1].disposition,
        CandidatePlacementDisposition::Rejected(_)
    ));
}

#[test]
fn absent_local_implementation_and_transport_increase_do_not_force_locality() {
    let (form, mut hosts, line) = fixture();
    let candidates = [
        candidate(&form, &hosts, "local", 0),
        candidate(&form, &hosts, "remote", 1),
    ];
    let current = basis(&form, &hosts, 80);
    let local_reduction = capability(&form, &hosts, "reduction", 0);
    hosts[0]
        .capabilities
        .retain(|offer| offer.capability_id != local_reduction);
    let selected = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &current,
        std::slice::from_ref(&line),
    )
    .expect("ordinary remote offer remains realizable");
    assert_eq!(selected.selected.candidate_id, "remote");
    assert!(matches!(
        selected.considered[0].disposition,
        CandidatePlacementDisposition::Rejected(_)
    ));

    let (form, hosts, line) = fixture();
    let candidates = [
        candidate(&form, &hosts, "local", 0),
        candidate(&form, &hosts, "remote", 1),
    ];
    let mut increasing = basis(&form, &hosts, 80);
    increasing.reductions[0].output_bytes_numerator = 2;
    increasing.reductions[0].input_bytes_denominator = 1;
    let selected = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &increasing,
        std::slice::from_ref(&line),
    )
    .expect("a reduction that increases traffic does not get a locality bonus");
    assert_eq!(selected.selected.candidate_id, "remote");
}

#[test]
fn policy_can_forbid_remote_transport_without_changing_form_meaning() {
    let (form, hosts, line) = fixture();
    let candidates = [
        candidate(&form, &hosts, "local-reduction", 0),
        candidate(&form, &hosts, "remote-reduction", 1),
    ];
    let mut policy = basis(&form, &hosts, 80);
    policy.remote_bytes_per_second_ceiling = Some(0);
    let checked_form_id = form.checked_form_id.clone();
    assert!(matches!(
        select_data_locality_candidate(
            &form,
            &hosts,
            &candidates,
            &policy,
            std::slice::from_ref(&line),
        ),
        Err(conduit_planner::PlannerError::CurrentResourceObservationUnavailable(_))
    ));
    assert_eq!(form.checked_form_id, checked_form_id);
}

#[test]
fn insufficient_local_resource_observation_moves_reduction_remote() {
    let (form, mut hosts, line) = fixture();
    let timer_requirement = hosts[0]
        .capabilities
        .iter()
        .find(|offer| {
            offer.checked_face()
                == form
                    .gears
                    .iter()
                    .find(|gear| gear.gear_id.as_str() == "source")
                    .unwrap()
                    .checked_face()
        })
        .unwrap()
        .resource_requirements[0]
        .clone();
    for host in &mut hosts {
        let reduction = form
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == "reduction")
            .unwrap();
        host.capabilities
            .iter_mut()
            .find(|offer| offer.checked_face() == reduction.checked_face())
            .unwrap()
            .resource_requirements
            .push(timer_requirement.clone());
    }
    let candidates = [
        candidate(&form, &hosts, "local", 0),
        candidate(&form, &hosts, "remote", 1),
    ];
    let mut constrained = basis(&form, &hosts, 80);
    constrained
        .resources
        .iter_mut()
        .find(|observation| {
            observation.host_id == hosts[0].host_id
                && observation.class_id == timer_requirement.class_id
        })
        .unwrap()
        .unreserved_units = 1;
    let selection = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &constrained,
        std::slice::from_ref(&line),
    )
    .expect("remote reduction remains within observed resources");
    assert_eq!(selection.selected.candidate_id, "remote");
    assert!(matches!(
        selection.considered[0].disposition,
        CandidatePlacementDisposition::Rejected(_)
    ));
}
