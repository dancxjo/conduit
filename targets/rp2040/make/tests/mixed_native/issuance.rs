//! CPU fault injection at the actual Root issuer, with already approved input.
use super::*;
use crate::i2c_issuance::Issuer;

pub fn check(
    context: b::NativePreparationContext<'_>,
    approved: &b::DelegatedBodyFragment<'_>,
    native: &ReadyI2cBase<Provider<'_>>,
) {
    let mut issuer = Issuer::new();
    let (issued, cost) = crate::probe::observe(|| issuer.issue(context, approved, native, 16384));
    let (mut table, mut handle, claim) = issued.unwrap();
    let inspection = table.inspections().next().unwrap();
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
    println!("actual Root I2C issuer: {} fallible allocations, {} requested B, {} B CPU peak; one-byte shortage and all allocation failures preserve provider; replacement table rejects old opaque handle; no bus effect", cost.allocations,cost.requested_bytes,cost.peak_bytes);
}
