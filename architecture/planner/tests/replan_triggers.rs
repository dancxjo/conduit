//! Ordinary pool evidence classifies pressure and loss without mutating a Plan.
#[path = "common/print_pool.rs"]
#[allow(dead_code)]
mod fixture;
use conduit_core::*;
use conduit_kernel::{shared_pool::MemberKey, NodeId};
use conduit_plan_lowering::shared_pool_runtime::*;
use conduit_process::{PrintColor, PrintMedia};
use fixture::*;
fn select(f: &PreparedPrinters, queue: i128, down: Option<usize>) -> PoolSelectionEvidence {
    let id = PoolOperationId::from("fixture/replan-operation");
    let costs = f.costs(&id, queue);
    let times = f.hosts.iter().map(|h| instant(h, 15)).collect::<Vec<_>>();
    let calibration = [revision()];
    let evaluations = f
        .hosts
        .iter()
        .zip(&times)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibration,
            executions: &[],
        })
        .collect::<Vec<_>>();
    let mut pool = HostedSharedPool::<2, 32, 4>::new(
        &f.plan,
        0,
        f.plan.fragments[0].shared_pools[0].pool_id.clone(),
        NodeId(10),
        1,
    )
    .unwrap();
    let outcome = pool
        .admit_new_operation_with_costs(
            id,
            MemberKey(f.demand),
            &f.observations(down),
            "fixture/replan-selection".into(),
            f.demand,
            &costs,
            &evaluations,
        )
        .unwrap();
    match outcome.admission {
        SharedPoolAdmissionOutcome::Selected { evidence, .. }
        | SharedPoolAdmissionOutcome::Refused(evidence) => evidence,
    }
}
fn replan(
    f: &PreparedPrinters,
    evidence: &PoolSelectionEvidence,
) -> Result<[ControlLoopEvent; 2], PoolSelectionEvidenceError> {
    let owner = &f.plan.fragments[0];
    evidence.exhaustion_replan_events(
        &f.plan,
        owner.host_id.clone(),
        owner.boot_id.clone(),
        PlanningRequestAuthority::HostLocal,
        "fixture/replan-request".into(),
    )
}
#[test]
fn ten_percent_queue_wobble_and_in_envelope_switch_keep_the_exact_plan() {
    let f = PreparedPrinters::prepare(request(PrintColor::Monochrome, PrintMedia::A4, 1));
    let original = f.plan.clone();
    for queue in [100, 110, 100, 90, 110] {
        let evidence = select(&f, queue, None);
        assert_eq!(evidence.disposition, PoolSelectionDisposition::Selected);
        assert_eq!(evidence.plan_id, f.plan.plan_id);
        evidence.validate(&f.plan).unwrap();
        assert_eq!(
            replan(&f, &evidence),
            Err(PoolSelectionEvidenceError::InvalidDisposition)
        );
    }
    let first = select(&f, 0, None);
    let fallback = select(
        &f,
        0,
        Some(usize::from(first.selected_realization.unwrap())),
    );
    assert_ne!(first.selected_realization, fallback.selected_realization);
    assert_eq!(fallback.plan_id, f.plan.plan_id);
    assert_eq!(
        replan(&f, &fallback),
        Err(PoolSelectionEvidenceError::InvalidDisposition)
    );
    assert_eq!(f.plan.clone(), original);
}
#[test]
fn actual_envelope_loss_emits_unsatisfied_then_planning_requested() {
    let f = PreparedPrinters::prepare(request(PrintColor::Monochrome, PrintMedia::A4, 1));
    let mut observations = f.observations(None);
    for observation in &mut observations {
        observation.health = PoolRealizationHealth::Unavailable;
        observation.sign_id = format!("fixture/lost/{}", observation.host_id.as_str()).into();
    }
    let id = PoolOperationId::from("fixture/lost-operation");
    let costs = f.costs(&id, 0);
    let times = f.hosts.iter().map(|h| instant(h, 15)).collect::<Vec<_>>();
    let calibration = [revision()];
    let evaluations = f
        .hosts
        .iter()
        .zip(&times)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibration,
            executions: &[],
        })
        .collect::<Vec<_>>();
    let mut pool = HostedSharedPool::<2, 32, 4>::new(
        &f.plan,
        0,
        f.plan.fragments[0].shared_pools[0].pool_id.clone(),
        NodeId(10),
        1,
    )
    .unwrap();
    let outcome = pool
        .admit_new_operation_with_costs(
            id,
            MemberKey(f.demand),
            &observations,
            "fixture/lost-selection".into(),
            f.demand,
            &costs,
            &evaluations,
        )
        .unwrap();
    let SharedPoolAdmissionOutcome::Refused(evidence) = outcome.admission else {
        panic!("all providers lost")
    };
    assert_eq!(
        evidence.disposition,
        PoolSelectionDisposition::EnvelopeExhausted
    );
    let events = replan(&f, &evidence).unwrap();
    assert!(
        matches!(&events[0], ControlLoopEvent::PlayBecameUnsatisfied { plan_id, reason: PlayUnsatisfiedReason::NoAdmittedPoolRealizationReady, .. } if plan_id == &f.plan.plan_id)
    );
    assert!(
        matches!(&events[1], ControlLoopEvent::PlanningRequested { prior_plan_id, .. } if prior_plan_id == &f.plan.plan_id)
    );
    for event in events {
        event.validate().unwrap();
    }
}
#[test]
fn occupied_capacity_is_pressure_and_cannot_request_a_replacement_plan() {
    let f = PreparedPrinters::prepare(request(PrintColor::Monochrome, PrintMedia::A4, 1));
    let times = f.hosts.iter().map(|h| instant(h, 15)).collect::<Vec<_>>();
    let calibration = [revision()];
    let evaluations = f
        .hosts
        .iter()
        .zip(&times)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibration,
            executions: &[],
        })
        .collect::<Vec<_>>();
    let mut pool = HostedSharedPool::<2, 32, 4>::new(
        &f.plan,
        0,
        f.plan.fragments[0].shared_pools[0].pool_id.clone(),
        NodeId(10),
        1,
    )
    .unwrap();
    let observations = f.observations(None);
    for index in 0..3_u8 {
        let id = PoolOperationId::from(format!("fixture/pressure/{index}").as_str());
        let costs = f.costs(&id, 0);
        let outcome = pool
            .admit_new_operation_with_costs(
                id,
                MemberKey([index; 32]),
                &observations,
                format!("fixture/pressure-selection/{index}").into(),
                f.demand,
                &costs,
                &evaluations,
            )
            .unwrap();
        match outcome.admission {
            SharedPoolAdmissionOutcome::Selected { member, .. } => {
                assert!(index < 2);
                pool.trigger(member).unwrap();
            }
            SharedPoolAdmissionOutcome::Refused(evidence) => {
                assert_eq!(index, 2);
                assert_eq!(
                    evidence.disposition,
                    PoolSelectionDisposition::CapacityRefused
                );
                assert_eq!(
                    replan(&f, &evidence),
                    Err(PoolSelectionEvidenceError::InvalidDisposition)
                );
                assert_eq!(evidence.plan_id, f.plan.plan_id);
            }
        }
    }
    assert!(observations
        .iter()
        .all(|o| o.health == PoolRealizationHealth::Ready));
}
