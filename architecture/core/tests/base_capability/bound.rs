use super::*;
use conduit_core::BoundBaseOperation;
fn ready() -> BoundBaseOperation {
    let mut table = table(7);
    let handle = table.issue(request()).unwrap();
    match BoundBaseOperation::bind(table, handle, claim()) {
        Ok(v) => v,
        Err((_, _, _, e)) => panic!("{e:?}"),
    }
}
#[test]
fn bound_operation_keeps_current_limits_rotation_revocation_and_exact_lease_checks() {
    let mut actual = ready();
    let first = actual.authorize().unwrap();
    let second = actual.authorize().unwrap();
    assert_eq!(actual.authorize(), Err(BaseCapabilityRefusal::InFlightFull));
    let wrong = first.clone();
    assert_eq!(
        actual.complete(wrong, 129),
        Err(BaseCapabilityRefusal::ResultEnvelope)
    );
    assert_eq!(actual.inspection().in_flight_operations, 2);
    actual.complete(first.clone(), 16).unwrap();
    assert_eq!(
        actual.complete(first, 16),
        Err(BaseCapabilityRefusal::UnknownLease)
    );
    actual.complete(second, 16).unwrap();
    let last = actual.authorize().unwrap();
    actual.complete(last, 16).unwrap();
    assert_eq!(actual.authorize(), Err(BaseCapabilityRefusal::Exhausted));
    actual.revoke().unwrap();
    assert_eq!(actual.authorize(), Err(BaseCapabilityRefusal::Revoked));
}
#[test]
fn full_claim_forgery_or_foreign_bearer_refuses_and_returns_every_input_unchanged() {
    for index in 0..14 {
        let mut table = table(7);
        let handle = table.issue(request()).unwrap();
        let mut invalid = claim();
        match index {
            0 => invalid.host_id = "wrong".into(),
            1 => invalid.boot_id = "wrong".into(),
            2 => invalid.base_instance_id = "wrong".into(),
            3 => invalid.base_provider_generation += 1,
            4 => invalid.plan_id = "wrong".into(),
            5 => invalid.active_play_id = "wrong".into(),
            6 => invalid.implementation_id = "wrong".into(),
            7 => invalid.operation_contract_id = "wrong".into(),
            8 => invalid.subject_kind = "wrong".into(),
            9 => invalid.resource_pool_id = "wrong".into(),
            10 => invalid.resource_generation_id = ResourceGenerationId("wrong".into()),
            11 => invalid.envelope_id = "wrong".into(),
            12 => invalid.parameter_bytes = 65,
            13 => invalid.work_units = 9,
            _ => unreachable!(),
        }
        let before = table.inspections().cloned().collect::<Vec<_>>();
        let original = invalid.clone();
        let (table, handle, invalid, error) = match BoundBaseOperation::bind(table, handle, invalid)
        {
            Err(v) => v,
            Ok(_) => panic!("forged claim"),
        };
        assert_eq!(invalid, original);
        assert_eq!(table.inspections().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            error,
            if index == 12 {
                BaseCapabilityRefusal::ParameterEnvelope
            } else if index == 13 {
                BaseCapabilityRefusal::WorkEnvelope
            } else {
                BaseCapabilityRefusal::WrongScope
            }
        );
        let mut bound = match BoundBaseOperation::bind(table, handle, claim()) {
            Ok(v) => v,
            Err(_) => panic!("ownership lost"),
        };
        let lease = bound.authorize().unwrap();
        bound.complete(lease, 16).unwrap();
    }
    let mut first = table(7);
    let _ = first.issue(request()).unwrap();
    let mut foreign = table(8);
    let handle = foreign.issue(request()).unwrap();
    let (_, handle, _, error) = match BoundBaseOperation::bind(first, handle, claim()) {
        Err(v) => v,
        Ok(_) => panic!("foreign bearer"),
    };
    assert_eq!(error, BaseCapabilityRefusal::UnknownCapability);
    foreign.authorize(&handle, &claim()).unwrap();
}
#[test]
fn old_claim_api_and_bound_api_have_identical_finite_trajectory_and_no_play_allocations() {
    let mut table = table(7);
    let mut handle = table.issue(request()).unwrap();
    let claim = claim();
    let mut bound = ready();
    super::allocation::without_allocations(|| {
        for _ in 0..3 {
            let old = table.authorize(&handle, &claim).unwrap();
            let new = bound.authorize().unwrap();
            table.complete(&mut handle, old, 16).unwrap();
            bound.complete(new, 16).unwrap();
            assert_eq!(table.inspections().next().unwrap(), bound.inspection());
        }
        assert_eq!(table.authorize(&handle, &claim), bound.authorize());
    });
}

#[test]
fn binding_releases_only_duplicate_claim_strings_without_an_allocation() {
    let mut table = table(7);
    let handle = table.issue(request()).unwrap();
    let claim = claim();
    let duplicate_bytes: usize = [
        claim.host_id.as_str(),
        claim.boot_id.as_str(),
        claim.base_instance_id.as_str(),
        claim.plan_id.as_str(),
        claim.active_play_id.as_str(),
        claim.implementation_id.as_str(),
        claim.operation_contract_id.as_str(),
        claim.subject_kind.as_str(),
        claim.resource_pool_id.as_str(),
        claim.resource_generation_id.0.as_str(),
        claim.envelope_id.as_str(),
    ]
    .iter()
    .map(|v| v.len())
    .sum();
    let (mut operation, released) =
        allocation::released_bytes(|| match BoundBaseOperation::bind(table, handle, claim) {
            Ok(operation) => operation,
            Err(_) => panic!("valid exclusive operation"),
        });
    assert_eq!(released, duplicate_bytes);
    println!("bound_operation_duplicate_claim_heap_released={released}");
    let lease = operation.authorize().unwrap();
    operation.complete(lease, 16).unwrap();
}

#[test]
fn revoked_bound_owner_rejects_late_leases_without_reviving_retired_work() {
    let mut actual = ready();
    let pending = actual.authorize().unwrap();
    actual.revoke().unwrap();
    let retired = actual.inspection().clone();
    assert_eq!(
        actual.complete(pending, 16),
        Err(BaseCapabilityRefusal::StaleCompletion)
    );
    assert_eq!(actual.authorize(), Err(BaseCapabilityRefusal::Revoked));
    assert_eq!(actual.inspection(), &retired);
    let mut replacement = ready();
    assert_eq!(replacement.inspection().completed_operations, 0);
    let next = replacement.authorize().unwrap();
    replacement.complete(next, 16).unwrap();
    assert_eq!(actual.inspection(), &retired);
}
