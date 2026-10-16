//! Resource loss changes later eligibility, never ownership of a dispatched job.
#[path = "common/print_pool.rs"]
#[allow(dead_code)]
mod fixture;
use conduit_core::*;
use conduit_kernel::{shared_pool::MemberKey, NodeId};
use conduit_plan_lowering::shared_pool_runtime::*;
use conduit_process::*;
use fixture::*;

#[test]
fn same_plan_resource_loss_fences_the_uncertain_job_and_routes_only_a_new_job() {
    let f = PreparedPrinters::prepare(request(PrintColor::Monochrome, PrintMedia::A4, 1));
    let plan_id = f.plan.plan_id.clone();
    let mut registry = PrintAttemptRegistry::<2>::prepare().unwrap();
    let attempt = identity("fixture/attempt-one");
    registry
        .admit(&f.request, &support(false), &binding(), attempt.clone())
        .unwrap();
    let pool_id = f.plan.fragments[0].shared_pools[0].pool_id.clone();
    let mut pool = HostedSharedPool::<2, 32, 4>::new(&f.plan, 0, pool_id, NodeId(10), 1).unwrap();
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
    let first_id = PoolOperationId::from(f.request.job_id().get().as_str());
    let costs = f.costs(&first_id, 0);
    let first = pool
        .admit_new_operation_with_costs(
            first_id.clone(),
            MemberKey(f.demand),
            &f.observations(None),
            "fixture/first-selection".into(),
            f.demand,
            &costs,
            &evaluations,
        )
        .unwrap();
    let SharedPoolAdmissionOutcome::Selected {
        member: first_member,
        ..
    } = first.admission
    else {
        panic!("expected first provider")
    };
    let index = usize::from(first_member.placement.realization);
    assert!(f.hosts[index].host_id.as_str().contains("mono-fast"));
    pool.trigger(first_member).unwrap();
    assert!(matches!(
        registry
            .observe(&attempt, PrintProviderReport::Dispatched)
            .unwrap(),
        PrintLifecycle::Dispatched(_)
    ));
    // Exact same Host/Boot/offer and sealed Plan; only a current slot Sign changes.
    let mut changed = f.observations(None);
    assert_eq!(changed[index].resources.len(), 1);
    changed[index].resources[0].health = ResourceHealth::Unavailable;
    changed[index].resources[0].unreserved_units = 0;
    changed[index].resources[0].sign_id = "fixture/slot-lost".into();
    changed[index].sign_id = "fixture/provider-resource-changed".into();
    let loss = pool
        .provider_lost(
            first_id,
            first_member,
            &changed,
            "fixture/first-loss".into(),
        )
        .unwrap();
    assert_eq!(loss.disposition, PoolSelectionDisposition::ProviderLost);
    loss.validate(&f.plan).unwrap();
    let uncertain = registry
        .observe(&attempt, PrintProviderReport::ProviderLost)
        .unwrap();
    assert!(matches!(uncertain, PrintLifecycle::Uncertain(_)));
    assert_eq!(
        pool.population(),
        1,
        "loss alone does not free an in-flight member"
    );
    assert_eq!(
        registry.admit(
            &f.request,
            &support(true),
            &binding(),
            identity("fixture/retry-on-color")
        ),
        Err(PrintRegistryRefusal::DuplicateJobId)
    );
    let second_request = PrintRequest::new(
        f.request.attributes().clone(),
        f.request.document().clone(),
        identity("fixture/job-two"),
    )
    .unwrap();
    registry
        .admit(
            &second_request,
            &support(true),
            &binding(),
            identity("fixture/attempt-two"),
        )
        .unwrap();
    let second_id = PoolOperationId::from(second_request.job_id().get().as_str());
    let second_demand = PreparedPrinters::prepare(second_request);
    assert_eq!(second_demand.plan.plan_id, plan_id);
    assert_ne!(second_demand.demand, f.demand);
    let costs = second_demand.costs(&second_id, 0);
    let second = pool
        .admit_new_operation_with_costs(
            second_id,
            MemberKey(second_demand.demand),
            &changed,
            "fixture/second-selection".into(),
            second_demand.demand,
            &costs,
            &evaluations,
        )
        .unwrap();
    let SharedPoolAdmissionOutcome::Selected {
        member: second_member,
        ..
    } = second.admission
    else {
        panic!("expected later eligible provider")
    };
    assert_ne!(
        first_member.placement.realization,
        second_member.placement.realization
    );
    assert!(f.hosts[usize::from(second_member.placement.realization)]
        .host_id
        .as_str()
        .contains("color-slow"));
    assert_eq!(f.plan.plan_id, plan_id);
    assert_eq!(
        registry.observe(
            &attempt,
            PrintProviderReport::Acknowledged(identity("fixture/late-ack"))
        ),
        Err(PrintRegistryRefusal::Protocol(
            PrintProtocolRefusal::InvalidTransition
        ))
    );
    // Fixture owner explicitly confirms retirement. It cannot erase uncertainty.
    pool.complete_release(first_member).unwrap();
    assert_eq!(pool.population(), 1);
    assert_eq!(
        registry.admit(
            &f.request,
            &support(true),
            &binding(),
            identity("fixture/after-retirement")
        ),
        Err(PrintRegistryRefusal::DuplicateJobId)
    );
}
