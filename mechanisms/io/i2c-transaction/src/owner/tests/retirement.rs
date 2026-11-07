use super::*;
use conduit_core::CapabilityLifecycle;

#[test]
fn retirement_preserves_issued_owner_then_returns_exact_revoked_custody_once() {
    let mut retained = Some(fixture::owner());
    let address = retained.as_ref().unwrap() as *const _;
    assert!(matches!(
        I2cHostCall::retire_retained(&mut retained),
        Err(I2cRetirementRefusal::NotRevoked)
    ));
    assert_eq!(retained.as_ref().unwrap() as *const _, address);
    let current = retained.as_mut().unwrap();
    let input = request(&I2cContract::prepare().unwrap(), 0x76, &[0xd0], 1);
    current
        .invoke(current.node, HostCallId(0), RequestId(0), &input)
        .unwrap();
    let node = current.node;
    assert_eq!(current.provider.effects, 1);
    assert_eq!(current.inspection().in_flight_operations, 0);
    current.revoke(node, HostCallId(0)).unwrap();
    assert_eq!(current.inspection().lifecycle, CapabilityLifecycle::Revoked);
    assert!(current
        .invoke(node, HostCallId(0), RequestId(1), &input)
        .is_err());
    let (provider, attachment) = I2cHostCall::retire_retained(&mut retained).unwrap();
    assert_eq!(provider.effects, 1);
    assert!(provider.revoked); // Returning custody must never undo quarantine.
    assert_eq!(attachment.generation, 7);
    assert_eq!(
        (attachment.minimum_address, attachment.maximum_address),
        (0x76, 0x77)
    );
    assert_eq!(attachment.resource_bytes, 4096);
    assert!(retained.is_none());
    assert!(matches!(
        I2cHostCall::<Provider>::retire_retained(&mut retained),
        Err(I2cRetirementRefusal::Unavailable)
    ));
}

#[test]
fn retirement_refuses_in_flight_possession_without_consuming_it() {
    let mut current = fixture::owner();
    let lease = current
        .table
        .authorize(&current.handle, &current.claim)
        .unwrap();
    let mut retained = Some(current);
    let address = retained.as_ref().unwrap() as *const _;
    assert!(matches!(
        I2cHostCall::retire_retained(&mut retained),
        Err(I2cRetirementRefusal::InFlight)
    ));
    assert_eq!(retained.as_ref().unwrap() as *const _, address);
    let current = retained.as_mut().unwrap();
    current
        .table
        .complete(&mut current.handle, lease, 0)
        .unwrap();
    assert!(matches!(
        I2cHostCall::retire_retained(&mut retained),
        Err(I2cRetirementRefusal::NotRevoked)
    ));
}

#[test]
fn exhaustion_is_not_revocation_and_does_not_release_custody() {
    let mut current = fixture::owner();
    let (fragment, _, active, _) = fixture::selected();
    let mut scope = fixture::scope();
    scope.maximum_operations = 1;
    let (table, handle, claim) = fixture::authorization(scope, &fragment, &active);
    current.table = table;
    current.handle = handle;
    current.claim = claim;
    let input = request(&I2cContract::prepare().unwrap(), 0x76, &[0xd0], 1);
    current
        .invoke(current.node, HostCallId(0), RequestId(0), &input)
        .unwrap();
    assert_eq!(
        current.inspection().lifecycle,
        CapabilityLifecycle::Exhausted
    );
    let mut retained = Some(current);
    assert!(matches!(
        I2cHostCall::retire_retained(&mut retained),
        Err(I2cRetirementRefusal::NotRevoked)
    ));
    let current = retained.as_mut().unwrap();
    current.revoke(current.node, HostCallId(0)).unwrap();
    let (provider, _) = I2cHostCall::retire_retained(&mut retained).unwrap();
    assert!(provider.revoked);
    assert_eq!(provider.effects, 1);
}

#[test]
fn clean_finish_returns_completed_idle_custody_without_quarantining_provider() {
    let mut current = fixture::owner();
    let input = request(&I2cContract::prepare().unwrap(), 0x76, &[0xd0], 1);
    current
        .invoke(current.node, HostCallId(0), RequestId(0), &input)
        .unwrap();
    let mut retained = Some(current);
    let (provider, attachment) = I2cHostCall::finish_retained(&mut retained).unwrap();
    assert_eq!(provider.effects, 1);
    assert!(!provider.revoked);
    assert_eq!(attachment.generation, 7);
    assert!(retained.is_none());
    assert!(matches!(
        I2cHostCall::<Provider>::finish_retained(&mut retained),
        Err(I2cRetirementRefusal::Unavailable)
    ));
}

#[test]
fn clean_finish_preserves_in_flight_custody_and_existing_quarantine() {
    let mut current = fixture::owner();
    let lease = current
        .table
        .authorize(&current.handle, &current.claim)
        .unwrap();
    let mut retained = Some(current);
    let address = retained.as_ref().unwrap() as *const _;
    assert!(matches!(
        I2cHostCall::finish_retained(&mut retained),
        Err(I2cRetirementRefusal::InFlight)
    ));
    assert_eq!(retained.as_ref().unwrap() as *const _, address);
    let current = retained.as_mut().unwrap();
    assert_eq!(current.inspection().in_flight_operations, 1);
    current
        .table
        .complete(&mut current.handle, lease, 0)
        .unwrap();
    current.revoke(current.node, HostCallId(0)).unwrap();
    let (provider, _) = I2cHostCall::finish_retained(&mut retained).unwrap();
    assert!(provider.revoked);
}
