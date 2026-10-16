//! Fixture printer protocols use the ordinary shared-pool allocator, not a spooler.
#[path = "common/print_pool.rs"]
mod fixture;
use conduit_kernel::{shared_pool::MemberKey, NodeId};
use conduit_plan_lowering::shared_pool_runtime::*;
use conduit_process::*;
use fixture::*;

fn select(f: &PreparedPrinters, queue: i128, down: Option<usize>) -> String {
    let id = "fixture/operation".into();
    let costs = f.costs(&id, queue);
    let now = f.hosts.iter().map(|h| instant(h, 15)).collect::<Vec<_>>();
    let calibration = [revision()];
    let evaluations = f
        .hosts
        .iter()
        .zip(&now)
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
    let admission = pool
        .admit_new_operation_with_costs(
            id,
            MemberKey([1; 32]),
            &f.observations(down),
            "fixture/selection".into(),
            f.demand,
            &costs,
            &evaluations,
        )
        .unwrap();
    let SharedPoolAdmissionOutcome::Selected { member, evidence } = admission.admission else {
        panic!("expected eligible provider")
    };
    evidence.validate(&f.plan).unwrap();
    pool.trigger(member).unwrap();
    let h = &f.hosts[usize::from(member.placement.realization)];
    let mut attempt = PrintAttempt::prepare(
        &f.request,
        &support(h.host_id.as_str().contains("color-slow")),
        &binding(),
        identity("fixture/attempt"),
    )
    .unwrap();
    assert!(matches!(
        attempt.observe(PrintProviderReport::Dispatched).unwrap(),
        PrintLifecycle::Dispatched(_)
    ));
    assert!(matches!(
        attempt
            .observe(PrintProviderReport::Acknowledged(identity(
                "fixture/provider-job"
            )))
            .unwrap(),
        PrintLifecycle::Acknowledged(_)
    ));
    assert!(matches!(
        attempt
            .observe(PrintProviderReport::Completed(identity(
                "fixture/provider-job"
            )))
            .unwrap(),
        PrintLifecycle::Completed(_)
    ));
    h.host_id.as_str().into()
}
#[test]
fn hard_attributes_precede_shared_cost_selection_and_exact_print_protocol() {
    let mono = PreparedPrinters::prepare(request(PrintColor::Monochrome, PrintMedia::A4, 1));
    assert!(select(&mono, 0, None).contains("mono-fast"));
    let color = PreparedPrinters::prepare(request(PrintColor::Color, PrintMedia::Letter, 1));
    assert_eq!(color.hosts.len(), 1);
    assert_eq!(color.refused[0].1, PrintAdmissionRefusal::UnsupportedColor);
    assert!(select(&color, 0, None).contains("color-slow"));
    let copies = PreparedPrinters::prepare(request(PrintColor::Monochrome, PrintMedia::A4, 3));
    assert_eq!(copies.hosts.len(), 1);
    assert!(select(&copies, 0, None).contains("color-slow"));
    for color in [false, true] {
        assert_eq!(
            admit_print_request(
                &request(PrintColor::Monochrome, PrintMedia::A3, 1),
                &support(color),
                &binding()
            ),
            Err(PrintAdmissionRefusal::UnsupportedMedia)
        );
    }
}
#[test]
fn fresh_queue_cost_and_provider_loss_change_selection_without_changing_plan() {
    let f = PreparedPrinters::prepare(request(PrintColor::Monochrome, PrintMedia::A4, 1));
    let id = f.plan.plan_id.clone();
    assert!(select(&f, 200, None).contains("color-slow"));
    let fast = f
        .hosts
        .iter()
        .position(|h| h.host_id.as_str().contains("mono-fast"))
        .unwrap();
    assert!(select(&f, 0, Some(fast)).contains("color-slow"));
    assert!(select(&f, 0, None).contains("mono-fast"));
    assert_eq!(f.plan.plan_id, id);
}
fn queued(f: &PreparedPrinters, id: &str, requester: &str, key: u8) -> QueuedPoolOperation {
    QueuedPoolOperation {
        plan: f.plan.plan_id.clone(),
        pool: f.plan.fragments[0].shared_pools[0].pool_id.clone(),
        operation: id.into(),
        requester: requester.into(),
        class: PoolWaitingClass::Batch,
        key: MemberKey([key; 32]),
        demand: f.demand,
        admitted_bytes: 4096,
        waiting_expires: instant(&f.hosts[0], 19),
    }
}
#[test]
fn generic_requester_turns_and_finite_waiting_survive_both_printers_being_busy() {
    let f = PreparedPrinters::prepare(request(PrintColor::Monochrome, PrintMedia::A4, 1));
    let pool_id = f.plan.fragments[0].shared_pools[0].pool_id.clone();
    let mut pool =
        HostedSharedPool::<2, 32, 4>::new(&f.plan, 0, pool_id.clone(), NodeId(10), 1).unwrap();
    let now = instant(&f.hosts[0], 15);
    pool.enable_waiting_with_policy(
        PoolWaitingLimits {
            operations: 3,
            per_requester: 2,
            admitted_bytes: 12288,
        },
        PoolWaitingPolicy::RequesterTurnWithAging {
            maximum_bypasses: 2,
        },
        now.clone(),
    )
    .unwrap();
    for (id, requester, key) in [("a/1", "a", 1), ("a/2", "a", 2), ("b/1", "b", 3)] {
        pool.queue_operation(queued(&f, id, requester, key), &now)
            .unwrap();
    }
    assert!(matches!(
        pool.queue_operation(queued(&f, "b/2", "b", 4), &now),
        Err(PoolWaitingError::Full)
    ));
    let observations = f.observations(None);
    let times = f.hosts.iter().map(|h| instant(h, 15)).collect::<Vec<_>>();
    let calibrations = [revision()];
    let evaluations = f
        .hosts
        .iter()
        .zip(&times)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibrations,
            executions: &[],
        })
        .collect::<Vec<_>>();
    let mut admitted = vec![];
    for id in ["a/1", "b/1"] {
        let costs = f.costs(&id.into(), 0);
        let selected = pool
            .admit_next_waiting_with_costs(
                &now,
                &observations,
                format!("fixture/select/{id}").into(),
                &costs,
                &evaluations,
            )
            .unwrap()
            .unwrap();
        assert_eq!(selected.retired.as_ref().unwrap().operation.as_str(), id);
        if id == "b/1" {
            assert_eq!(
                selected.decision.reason,
                PoolWaitingChoiceReason::RequesterTurn
            );
        }
        let SharedPoolAdmissionOutcome::Selected { member, .. } = selected.outcome.admission else {
            panic!("expected selected")
        };
        pool.trigger(member).unwrap();
        admitted.push(member);
    }
    let costs = f.costs(&"a/2".into(), 0);
    let refused = pool
        .admit_next_waiting_with_costs(
            &now,
            &observations,
            "fixture/full".into(),
            &costs,
            &evaluations,
        )
        .unwrap()
        .unwrap();
    assert!(
        matches!(refused.outcome.admission,SharedPoolAdmissionOutcome::Refused(e) if e.disposition==conduit_core::PoolSelectionDisposition::CapacityRefused)
    );
    assert!(refused.retired.is_none());
    assert_eq!(pool.waiting_population(), 1);
    // Only explicit completion releases capacity; waiting never retries an admitted job.
    pool.release(admitted[0]).unwrap();
    let selected = pool
        .admit_next_waiting_with_costs(
            &now,
            &observations,
            "fixture/resumed".into(),
            &costs,
            &evaluations,
        )
        .unwrap()
        .unwrap();
    assert!(matches!(
        selected.outcome.admission,
        SharedPoolAdmissionOutcome::Selected { .. }
    ));
    assert_eq!(selected.retired.unwrap().operation.as_str(), "a/2");
    assert_eq!(pool.waiting_population(), 0);
    let mut bounded =
        HostedSharedPool::<2, 32, 4>::new(&f.plan, 0, pool_id, NodeId(10), 2).unwrap();
    bounded
        .enable_waiting(
            PoolWaitingLimits {
                operations: 4,
                per_requester: 1,
                admitted_bytes: 8192,
            },
            now.clone(),
        )
        .unwrap();
    bounded
        .queue_operation(queued(&f, "a/1", "a", 1), &now)
        .unwrap();
    assert!(matches!(
        bounded.queue_operation(queued(&f, "a/2", "a", 2), &now),
        Err(PoolWaitingError::RequesterFull)
    ));
    bounded
        .queue_operation(queued(&f, "b/1", "b", 3), &now)
        .unwrap();
    assert!(matches!(
        bounded.queue_operation(queued(&f, "c/1", "c", 4), &now),
        Err(PoolWaitingError::ByteLimit)
    ));
    assert_eq!(bounded.waiting_population(), 2);
}
#[test]
fn expired_cost_is_excluded_and_generic_capacity_fallback_is_explicit() {
    let f = PreparedPrinters::prepare(request(PrintColor::Monochrome, PrintMedia::A4, 1));
    let id = "fixture/expired".into();
    let costs = f.costs(&id, 0);
    let times = f.hosts.iter().map(|h| instant(h, 21)).collect::<Vec<_>>();
    let calibrations = [revision()];
    let evaluations = f
        .hosts
        .iter()
        .zip(&times)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibrations,
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
    let refused = pool
        .admit_new_operation_with_costs(
            id,
            MemberKey([4; 32]),
            &f.observations(None),
            "fixture/expired-selection".into(),
            f.demand,
            &costs,
            &evaluations,
        )
        .unwrap();
    assert!(matches!(
        refused.admission,
        SharedPoolAdmissionOutcome::Selected { .. }
    ));
    assert!(refused.account.sources.iter().all(|s| s.availability
        == conduit_core::PoolCostSourceAvailability::Unavailable(
            conduit_core::PoolCostUnavailable::Prepared(
                conduit_core::PoolCostPreparationRefusal::Stale
            )
        )));
    assert_eq!(
        refused.account.choice,
        Some(conduit_core::PoolCostChoiceBasis::CapacityReference)
    );
    refused.account.validate(&f.plan).unwrap();
    assert_eq!(pool.population(), 1);
}
