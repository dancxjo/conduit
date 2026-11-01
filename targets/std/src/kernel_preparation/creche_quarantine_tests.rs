//! Library-only Crèche reservation custody regression.
use super::*;
#[test]
fn exact_creche_reservation_hold_moves_without_allocation_or_release() {
    let mut adapter = crate::hosted_creche::test_support::Adapter::new();
    let (host, plan, _, _) = crate::hosted_creche::test_support::planned(&adapter);
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    let tokens = ledger
        .prepare_and_reserve_plans(&host, &[(&plan.plots[0].plan, false)])
        .unwrap();
    let token_storage = tokens.as_ptr();
    let before_pools = ledger.pools.clone();
    let before_instances = ledger.instances.clone();
    let allocations = crate::allocation_probe::begin();
    ledger.quarantine_retirement(tokens);
    assert_eq!(allocations.finish(), 0);
    assert_eq!(
        ledger.retirement_hold.as_ref().unwrap().as_ptr(),
        token_storage
    );
    assert_eq!(
        ledger.retirement_hold.as_ref().unwrap()[0].plan_id,
        plan.plots[0].plan.plan_id
    );
    assert_eq!(ledger.instances, before_instances);
    for (actual, before) in ledger.pools.iter().zip(&before_pools) {
        assert_eq!(actual.used_units, before.used_units);
    }
    assert!(ledger
        .prepare_capability_refresh(&host, &host, None)
        .is_err());
    // A private adapter state change cannot clear the charged Host hold.
    adapter.fail_drain = false;
    assert!(ledger.require_available().is_err());
}
