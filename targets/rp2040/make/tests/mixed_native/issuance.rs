//! CPU fault injection at the actual Root issuer, with already approved input.
use super::*;
use crate::i2c_issuance::Issuer;

pub fn check(
    context: b::NativePreparationContext<'_>,
    approved: &b::DelegatedBodyFragment<'_>,
    native: &ReadyI2cBase<Provider<'_>>,
) {
    let mut issuer = Issuer::new();
    let (missing, missing_cost) =
        crate::probe::observe(|| issuer.issue(context, approved, native, 16384));
    assert!(matches!(
        missing,
        Err(crate::native_preparation::Refusal::Scope)
    ));
    assert_eq!(
        (missing_cost.allocations, missing_cost.reallocations),
        (0, 0)
    );
    let (short, short_cost) = crate::probe::observe(|| issuer.authorize(context, native, 0));
    assert!(matches!(
        short,
        Err(crate::native_preparation::Refusal::Bounds)
    ));
    assert_eq!((short_cost.allocations, short_cost.reallocations), (0, 0));
    assert!(issuer.grant().is_none());
    let (authorized, grant_cost) =
        crate::probe::observe(|| issuer.authorize(context, native, 16384));
    authorized.unwrap();
    assert!(issuer.grant().is_some());
    println!("original Root grant: {} fallible allocations, {} requested B, {} B CPU peak; descriptor only, no bus lease or opaque capability", grant_cost.allocations, grant_cost.requested_bytes, grant_cost.peak_bytes);
    for nth in 1..=grant_cost.allocations {
        let mut refused = Issuer::new();
        let (result, failed) =
            crate::probe::observe_refusing(nth, || refused.authorize(context, native, 16384));
        assert!(result.is_err());
        assert!(refused.grant().is_none());
        assert_eq!(failed.refused_allocations, 1);
        assert_eq!(failed.allocations, failed.deallocations);
    }
    let original = issuer.grant().unwrap().clone();
    let (again, repeated) = crate::probe::observe(|| issuer.authorize(context, native, 0));
    again.unwrap();
    assert_eq!((repeated.allocations, repeated.reallocations), (0, 0));
    assert_eq!(issuer.grant(), Some(&original));
    let mut foreign_member = context.member.clone();
    foreign_member.body_id = serde_json::from_str("\"foreign/body\"").unwrap();
    assert!(issuer
        .authorize(
            b::NativePreparationContext {
                member: &foreign_member,
                ..context
            },
            native,
            16384
        )
        .is_err());
    let mut changed = context.actual.clone();
    changed.offer_generation.0 += 1;
    assert!(issuer
        .authorize(
            b::NativePreparationContext {
                actual: &changed,
                ..context
            },
            native,
            16384
        )
        .is_err());
    assert_eq!(issuer.grant(), Some(&original));
    let (copy_result, copy_cost) = crate::probe::observe(|| issuer.copy_grants_for_return(16384));
    let copied = copy_result.unwrap();
    for nth in 1..=copy_cost.allocations {
        let (result, failed) =
            crate::probe::observe_refusing(nth, || issuer.copy_grants_for_return(16384));
        assert!(result.is_err());
        assert_eq!(failed.refused_allocations, 1);
        assert_eq!(failed.allocations, failed.deallocations);
        assert_eq!(issuer.grant(), Some(&original));
    }
    assert_eq!(copied.as_slice(), std::slice::from_ref(&original));
    assert!(issuer.copy_grants_for_return(0).is_err());
    let (issued, cost) = crate::probe::observe(|| issuer.issue(context, approved, native, 16384));
    let (mut table, mut handle, claim) = issued.unwrap();
    let inspection = table.inspections().next().unwrap();
    assert_eq!(inspection.scope.authority_grant_id, original.grant_id);
    assert_eq!(inspection.scope.maximum_operations, 1024);
    assert_eq!(inspection.scope.maximum_work_units, 1);
    assert_eq!(
        inspection.scope.plan_id.as_str(),
        approved.metadata().body_plan
    );
    assert_eq!(
        inspection.scope.active_play_id.as_str(),
        approved.metadata().body_play
    );
    assert_eq!(
        inspection.scope.base_instance_id,
        native.native_identity().provider_instance_id
    );
    assert_eq!(inspection.scope.maximum_in_flight, 1);
    let (short, shortage) =
        crate::probe::observe(|| issuer.issue(context, approved, native, cost.requested_bytes - 1));
    assert!(matches!(
        short,
        Err(crate::native_preparation::Refusal::Bounds)
    ));
    assert_eq!((shortage.allocations, shortage.reallocations), (0, 0));
    for nth in 1..=cost.allocations {
        let (result, failed) = crate::probe::observe_refusing(nth, || {
            issuer.issue(context, approved, native, cost.requested_bytes)
        });
        assert!(result.is_err(), "allocation {nth} was not refused");
        assert_eq!(failed.refused_allocations, 1);
        assert_eq!(failed.allocations, failed.deallocations);
    }
    let (mut replacement, _, replacement_claim) = issuer
        .issue(context, approved, native, cost.requested_bytes)
        .unwrap();
    assert_eq!(
        replacement.authorize(&handle, &replacement_claim),
        Err(BaseCapabilityRefusal::UnknownCapability)
    );
    let lease = table.authorize(&handle, &claim).unwrap();
    table.complete(&mut handle, lease, 0).unwrap();
    let mut wrong = context;
    wrong.image_id = "wrong/image";
    let (result, rejected) = crate::probe::observe(|| issuer.issue(wrong, approved, native, 16384));
    assert!(matches!(
        result,
        Err(crate::native_preparation::Refusal::Scope)
    ));
    assert_eq!((rejected.allocations, rejected.reallocations), (0, 0));
    issuer.withdraw_authorization();
    assert!(issuer.grant().is_none());
    assert!(issuer.issue(context, approved, native, 16384).is_err());
    assert_eq!(
        copied[0], original,
        "a retained description cannot restore withdrawn Root authority"
    );
    println!("actual Root I2C issuer: {} fallible allocations, {} requested B, {} B CPU peak; one-byte shortage and all allocation failures preserve provider; replacement table rejects old opaque handle; no bus effect", cost.allocations,cost.requested_bytes,cost.peak_bytes);
}
