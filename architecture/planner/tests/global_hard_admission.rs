//! Global search consumes ordinary hard gates; absent cost is never numeric zero.
mod common;
#[path = "common/global_connected_fixture.rs"]
mod global_connected_fixture;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use std::collections::BTreeSet;

#[path = "common/global_hard_fixture.rs"]
mod global_hard_fixture;
use global_hard_fixture::exercise;

#[test]
fn exact_host_affinity_excludes_otherwise_admitted_candidates() {
    exercise(
        |hosts, requirements| {
            requirements.predicates.push(PlannerPredicate::Equal {
                fact: PlannerFactRef::HostIdentity,
                value: PlannerFactValue::Category(hosts[1].host_id.as_str().into()),
            })
        },
        |_| {},
        |result, hosts| {
            let result = result.unwrap();
            assert_eq!(result.candidate_count, 1);
            assert_eq!(
                result.plan.fragments[0].placements[0].host_id,
                hosts[1].host_id
            );
        },
    );
}

#[test]
fn finite_queue_and_resource_ceiling_filter_before_global_search() {
    exercise(
        |_, requirements| requirements.minimum_queue_items = 8,
        |_| {},
        |result, hosts| {
            let result = result.unwrap();
            assert_eq!(result.candidate_count, 1);
            assert_eq!(
                result.plan.fragments[0].placements[0].host_id,
                hosts[1].host_id
            );
        },
    );
    exercise(
        |hosts, requirements| {
            requirements.maximum_resource_units.insert(
                hosts[0].capabilities[0].resource_requirements[0]
                    .class_id
                    .clone(),
                1,
            );
        },
        |_| {},
        |result, hosts| {
            let result = result.unwrap();
            assert_eq!(result.candidate_count, 1);
            assert_eq!(
                result.plan.fragments[0].placements[0].host_id,
                hosts[0].host_id
            );
        },
    );
}

#[test]
fn exhausted_capacity_and_wrong_boot_observations_cannot_be_reserved() {
    for wrong_boot in [false, true] {
        exercise(
            |_, _| {},
            |observations| {
                for observation in observations {
                    if wrong_boot {
                        observation.boot_id = "foreign/boot".into();
                    } else {
                        observation.unreserved_units = 0;
                    }
                }
            },
            |result, _| {
                if wrong_boot {
                    assert!(matches!(
                        result,
                        Err(GlobalPlacementRefusal::Candidate(
                            CostSelectionError::Evidence(CostEvidenceRefusal::InvalidClock(
                                MonotonicTimeRefusal::DifferentClock
                            ))
                        ))
                    ));
                } else {
                    assert_hard_refusal(result, RealizationRejection::CurrentResourceObservation);
                }
            },
        );
    }
}

#[test]
fn declared_authority_cannot_be_bypassed_by_global_search() {
    exercise(
        |hosts, requirements| {
            for host in hosts {
                let offer = &mut host.capabilities[0];
                offer.authority_requirements.push(AuthorityRequirement {
                    contract_id: "global-hard/authority@1".into(),
                    host_call_contract_id: WAIT_HOST_CALL_CONTRACT.into(),
                    subject_kind: offer.kind_id.clone(),
                });
            }
            requirements.permitted_authority_contracts = Some(BTreeSet::new());
        },
        |_| {},
        |result, _| assert_hard_refusal(result, RealizationRejection::AuthorityContractAllowlist),
    );
}

#[test]
fn incompatible_front_is_never_rescued_by_the_global_assignment_search() {
    for all in [false, true] {
        exercise(
            |hosts, _| {
                hosts[0].capabilities[0].outputs[0].value_kind = kind_id("value/u64");
                if all {
                    hosts[1].capabilities[0].outputs[0].value_kind = kind_id("value/u64");
                }
            },
            |_| {},
            |result, hosts| {
                if all {
                    let Err(GlobalPlacementRefusal::Candidate(CostSelectionError::HardAdmission(
                        refusal,
                    ))) = result
                    else {
                        panic!("incompatible Front accepted");
                    };
                    assert!(matches!(refusal.reason, PlannerError::UnknownCapability(_)));
                } else {
                    let result = result.unwrap();
                    assert_eq!(result.candidate_count, 1);
                    assert_eq!(
                        result.plan.fragments[0].placements[0].host_id,
                        hosts[1].host_id
                    );
                }
            },
        );
    }
}

fn assert_hard_refusal(
    result: Result<GlobalPlacementResult, GlobalPlacementRefusal>,
    reason: RealizationRejection,
) {
    let Err(GlobalPlacementRefusal::Candidate(CostSelectionError::HardAdmission(refusal))) = result
    else {
        panic!("expected named hard-admission refusal");
    };
    assert_eq!(refusal.signs.len(), 2);
    assert!(refusal
        .signs
        .iter()
        .all(|sign| sign.disposition == RealizationDecisionDisposition::Rejected(reason.clone())));
}

#[test]
fn global_assignment_reserves_declared_memory_and_typed_compute_limits() {
    exercise(
        |hosts, _| {
            for (index, host) in hosts.iter_mut().enumerate() {
                host.resources.push(resource_offer(
                    "global-hard/memory",
                    "global-hard/memory-bytes",
                    if index == 0 { 1024 } else { 4096 },
                ));
                host.capabilities[0]
                    .resource_requirements
                    .push(resource_requirement("global-hard/memory-bytes", 2048));
            }
        },
        |_| {},
        |result, hosts| {
            let result = result.unwrap();
            assert_eq!(
                result.plan.fragments[0].placements[0].host_id,
                hosts[1].host_id
            );
            let binding = result.plan.fragments[0].placements[0]
                .resources
                .iter()
                .find(|r| r.class_id.as_str() == "global-hard/memory-bytes")
                .unwrap();
            assert_eq!(binding.units, 2048);
        },
    );
    exercise(
        |hosts, _| {
            for (index, host) in hosts.iter_mut().enumerate() {
                host.resources.push(compute_resource_offer(
                    "global-hard/cpu",
                    "global-hard/cpu-lanes",
                    if index == 0 { 1 } else { 2 },
                    ComputePoolContract {
                        service_guarantee: ComputeServiceGuarantee::Shared,
                        architecture_base_id: "global-hard/base".into(),
                        architecture_base_kind: ArchitectureBaseKind::HostedOs,
                        topology_groups: vec![],
                    },
                ));
                host.capabilities[0]
                    .resource_requirements
                    .push(compute_resource_requirement(
                        "global-hard/cpu-lanes",
                        2,
                        2,
                        2,
                        ComputeServiceGuarantee::Shared,
                        None,
                    ));
            }
        },
        |_| {},
        |result, hosts| {
            let result = result.unwrap();
            assert_eq!(
                result.plan.fragments[0].placements[0].host_id,
                hosts[1].host_id
            );
            let binding = result.plan.fragments[0].placements[0]
                .resources
                .iter()
                .find(|r| r.class_id.as_str() == "global-hard/cpu-lanes")
                .unwrap();
            assert_eq!(binding.units, 2);
            assert_eq!(binding.compute.as_ref().unwrap().selected_lanes, 2);
        },
    );
}

#[test]
fn connected_global_search_refuses_foreign_unavailable_and_unbounded_lines() {
    use global_connected_fixture::connected_with_lines;
    let baseline = global_connected_fixture::connected(false, false, false).unwrap();
    assert!(verify_plan(&baseline.plan));
    assert!(baseline.states <= 128);
    for mutation in 0..7 {
        let result = connected_with_lines(false, false, false, |lines| match mutation {
            0 => lines[0].binding.source.boot_id = "foreign/source-boot".into(),
            1 => lines[0].binding.sink.boot_id = "foreign/sink-boot".into(),
            2 => lines[0].binding.base = "unsupported/transport@1".into(),
            3 => lines[0].binding.limits.maximum_payload_bytes = 1,
            4 => lines[0].availability.binding_id = "foreign/binding".into(),
            5 => lines[0].binding.limits.maximum_in_flight_items = 0,
            6 => lines[0].availability.availability = LineAvailability::Unavailable,
            _ => unreachable!(),
        });
        assert!(
            matches!(
                result,
                Err(GlobalPlacementRefusal::NoFeasibleAssignment {
                    disposition: GlobalPlacementDisposition::Exhaustive,
                    ..
                })
            ),
            "mutation {mutation} did not refuse in ordinary sealing: {result:?}"
        );
        assert!(verify_plan(&baseline.plan));
    }
}

#[test]
fn stale_offer_generation_is_not_a_current_capacity_observation() {
    exercise(
        |_, _| {},
        |observations| {
            for observation in observations {
                observation.offer_generation = OfferGeneration(observation.offer_generation.0 + 1);
            }
        },
        |result, _| {
            let Err(GlobalPlacementRefusal::Candidate(CostSelectionError::HardAdmission(refusal))) =
                result
            else {
                panic!("stale generation reached search");
            };
            assert!(matches!(
                refusal.reason,
                PlannerError::InvalidResourceObservation(_)
            ));
            assert!(
                refusal.signs.is_empty(),
                "validation must refuse before candidate ranking"
            );
        },
    );
}

#[test]
fn allowlisted_authority_still_requires_exact_grants_at_plan_sealing() {
    exercise(
        |hosts, _| {
            for host in hosts {
                let offer = &mut host.capabilities[0];
                offer.authority_requirements.push(AuthorityRequirement {
                    contract_id: "global-hard/authority@1".into(),
                    host_call_contract_id: WAIT_HOST_CALL_CONTRACT.into(),
                    subject_kind: offer.kind_id.clone(),
                });
            }
        },
        |_| {},
        |result, _| {
            assert!(
                matches!(
                    result,
                    Err(GlobalPlacementRefusal::NoFeasibleAssignment {
                        disposition: GlobalPlacementDisposition::Exhaustive,
                        refused_assignments: 2,
                        ..
                    })
                ),
                "authority without grants reached a sealed Plan: {result:?}"
            );
        },
    );
}
