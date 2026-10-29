//! Borrowing transient authority metadata changes no issuance or refusal fact.
use super::*;
use conduit_core::BaseCapabilityAuthorityRef;

#[test]
fn owned_and_borrowed_authority_issue_identical_possession_and_lifetime() {
    let mut owned = table(7);
    let mut borrowed = table(7);
    let policy = authority();
    let mut first = owned.issue(request()).unwrap();
    let mut second = borrowed.issue_borrowed(scope(), (&policy).into()).unwrap();
    assert!(first == second);
    assert_eq!(
        owned.inspections().collect::<Vec<_>>(),
        borrowed.inspections().collect::<Vec<_>>()
    );
    for _ in 0..3 {
        let a = owned.authorize(&first, &claim()).unwrap();
        let b = borrowed.authorize(&second, &claim()).unwrap();
        assert!(a == b);
        owned.complete(&mut first, a, 32).unwrap();
        borrowed.complete(&mut second, b, 32).unwrap();
        assert!(first == second);
        assert_eq!(
            owned.inspections().collect::<Vec<_>>(),
            borrowed.inspections().collect::<Vec<_>>()
        );
    }
    assert_eq!(
        owned.authorize(&first, &claim()).err(),
        Some(BaseCapabilityRefusal::Exhausted)
    );
    assert_eq!(
        borrowed.authorize(&second, &claim()).err(),
        Some(BaseCapabilityRefusal::Exhausted)
    );
}

#[test]
fn borrowed_authority_refuses_every_mismatched_authority_field_and_bound() {
    for mutation in 0..24 {
        let mut input = request();
        let expected = match mutation {
            0 => {
                input.scope.plan_id = "".into();
                BaseCapabilityRefusal::EmptyIdentity
            }
            1 => {
                input.scope.maximum_work_units = 0;
                BaseCapabilityRefusal::InvalidBound
            }
            2 => {
                input.authority.grant.host_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            3 => {
                input.authority.grant.boot_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            4 => {
                input.authority.grant.grant_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            5 => {
                input.authority.grant.contract_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            6 => {
                input.authority.grant.capability_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            7 => {
                input.authority.grant.host_call_contract_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            8 => {
                input.authority.grant.subject_kind = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            9 => {
                input.authority.base_instance_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            10 => {
                input.authority.base_provider_generation += 1;
                BaseCapabilityRefusal::AuthorityMismatch
            }
            11 => {
                input.authority.operation_contract_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            12 => {
                input.authority.resource_pool_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            13 => {
                input.authority.resource_generation_id = ResourceGenerationId("other".into());
                BaseCapabilityRefusal::AuthorityMismatch
            }
            14 => {
                input.authority.envelope_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            15 => {
                input.scope.maximum_parameter_bytes = input.authority.maximum_parameter_bytes + 1;
                BaseCapabilityRefusal::ScopeBroadening
            }
            16 => {
                input.scope.maximum_result_bytes = input.authority.maximum_result_bytes + 1;
                BaseCapabilityRefusal::ScopeBroadening
            }
            17 => {
                input.scope.maximum_work_units = input.authority.maximum_work_units + 1;
                BaseCapabilityRefusal::ScopeBroadening
            }
            18 => {
                input.scope.maximum_in_flight = input.authority.maximum_in_flight + 1;
                BaseCapabilityRefusal::ScopeBroadening
            }
            19 => {
                input.scope.maximum_operations = input.authority.maximum_operations + 1;
                BaseCapabilityRefusal::ScopeBroadening
            }
            20 => {
                input.scope.host_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            21 => {
                input.scope.boot_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            22 => {
                input.scope.base_instance_id = "other".into();
                BaseCapabilityRefusal::AuthorityMismatch
            }
            23 => {
                input.scope.base_provider_generation += 1;
                BaseCapabilityRefusal::AuthorityMismatch
            }
            _ => unreachable!(),
        };
        let mut owned = table(7);
        let mut borrowed = table(7);
        assert_eq!(
            owned.issue(input.clone()).err(),
            Some(expected.clone()),
            "owned mutation {mutation}"
        );
        assert_eq!(
            borrowed
                .issue_borrowed(input.scope, (&input.authority).into())
                .err(),
            Some(expected),
            "borrowed mutation {mutation}"
        );
        assert_eq!(owned.inspections().count(), 0);
        assert_eq!(borrowed.inspections().count(), 0);
        // Refusal does not consume issuance identity or create possession.
        assert!(owned.issue(request()).unwrap() == borrowed.issue(request()).unwrap());
    }
}

#[test]
fn owned_and_borrowed_refusal_order_and_authority_lifetime_are_unchanged() {
    let mut owned = table(7);
    let mut borrowed = table(7);
    let mut policy = authority();
    for _ in 0..4 {
        assert!(
            owned.issue(request()).unwrap()
                == borrowed.issue_borrowed(scope(), (&policy).into()).unwrap()
        );
    }
    let mut invalid = request();
    invalid.scope.plan_id = "".into();
    invalid.scope.maximum_work_units = 0;
    invalid.authority.grant.host_id = "other".into();
    assert_eq!(
        owned.issue(invalid.clone()).err(),
        Some(BaseCapabilityRefusal::TableFull)
    );
    assert_eq!(
        borrowed
            .issue_borrowed(invalid.scope.clone(), (&invalid.authority).into())
            .err(),
        Some(BaseCapabilityRefusal::TableFull)
    );
    let mut empty = table(7);
    assert_eq!(
        empty
            .issue_borrowed(invalid.scope.clone(), (&invalid.authority).into())
            .err(),
        Some(BaseCapabilityRefusal::EmptyIdentity)
    );
    invalid.scope.plan_id = "valid".into();
    assert_eq!(
        empty
            .issue_borrowed(invalid.scope.clone(), (&invalid.authority).into())
            .err(),
        Some(BaseCapabilityRefusal::InvalidBound)
    );
    invalid.scope.maximum_work_units = 1000;
    assert_eq!(
        empty
            .issue_borrowed(invalid.scope, (&invalid.authority).into())
            .err(),
        Some(BaseCapabilityRefusal::AuthorityMismatch)
    );
    let handle = empty.issue_borrowed(scope(), (&policy).into()).unwrap();
    policy.grant.host_id = "retired-policy-storage".into();
    assert_eq!(
        empty.inspections().next().unwrap().scope.host_id.as_str(),
        "host/one"
    );
    assert!(empty.authorize(&handle, &claim()).is_ok());
    allocation::without_allocations(|| {
        let projected: BaseCapabilityAuthorityRef<'_> = (&policy).into();
        assert_eq!(projected.grant.host_id, "retired-policy-storage");
    });
}
