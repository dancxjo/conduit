use conduit_kernel::shared_pool::{
    FixedSharedPool, MemberIdentity, MemberKey, MemberPlacement, PoolError, PoolId, PoolSignReason,
};
use conduit_kernel::NodeId;

fn key(value: u8) -> MemberKey {
    MemberKey([value; 32])
}

fn placement(value: u16) -> MemberPlacement {
    MemberPlacement {
        node: NodeId(value),
        realization: value,
        play: value + 10,
    }
}

fn placeholder() -> MemberIdentity {
    MemberIdentity {
        pool: PoolId(u16::MAX),
        key: key(0),
        slot: u16::MAX,
        epoch: u32::MAX,
        placement: placement(0),
    }
}

#[test]
fn keyed_membership_is_finite_and_slot_reuse_rejects_stale_epochs() {
    let mut pool = FixedSharedPool::<2, 16>::new(PoolId(4), 2, 7, 2).unwrap();
    let first = pool.admit(key(2), placement(1), 7).unwrap();
    pool.trigger(first).unwrap();
    let second = pool.admit(key(1), placement(0), 7).unwrap();
    pool.trigger(second).unwrap();
    assert_eq!(pool.active_population(), 2);
    assert_eq!(
        pool.admit(key(3), placement(0), 7),
        Err(PoolError::PoolFull)
    );
    assert_eq!(
        pool.admit(key(2), placement(1), 7),
        Err(PoolError::DuplicateKey)
    );

    pool.request_release(first).unwrap();
    pool.complete_release(first).unwrap();
    let replacement = pool.admit(key(2), placement(1), 7).unwrap();
    assert_eq!(replacement.slot, first.slot);
    assert_eq!(replacement.epoch, first.epoch + 1);
    assert_eq!(pool.validate_active(first), Err(PoolError::StaleMember));
    pool.trigger(replacement).unwrap();
    assert_eq!(pool.member_for_key(key(2)).unwrap(), replacement);

    let reasons = pool.signs().map(|event| event.reason).collect::<Vec<_>>();
    assert!(reasons.contains(&PoolSignReason::PoolFull));
    assert!(reasons.contains(&PoolSignReason::DuplicateKey));
    assert!(reasons.contains(&PoolSignReason::Released));
}

#[test]
fn authority_realization_preparation_and_lifecycle_fail_independently() {
    let mut pool = FixedSharedPool::<2, 16>::new(PoolId(4), 2, 7, 1).unwrap();
    assert_eq!(
        pool.admit(key(1), placement(0), 8),
        Err(PoolError::AuthorityDenied)
    );
    assert_eq!(
        pool.admit(key(1), placement(1), 7),
        Err(PoolError::RealizationDenied)
    );
    let failed = pool.admit(key(1), placement(0), 7).unwrap();
    pool.fail_preparation(failed).unwrap();
    assert_eq!(pool.population(), 0);
    assert_eq!(pool.trigger(failed), Err(PoolError::InvalidLifecycle));
    assert_eq!(pool.member_for_key(key(9)), Err(PoolError::UnknownKey));

    let retried = pool.admit(key(1), placement(0), 7).unwrap();
    assert!(retried.epoch > failed.epoch);
    pool.trigger(retried).unwrap();
    assert_eq!(pool.trigger(retried), Err(PoolError::InvalidLifecycle));
}

#[test]
fn fan_snapshot_is_key_ordered_and_does_not_change_when_membership_changes() {
    let mut pool = FixedSharedPool::<3, 16>::new(PoolId(4), 3, 7, 3).unwrap();
    for value in [3, 1] {
        let member = pool
            .admit(key(value), placement(u16::from(value - 1)), 7)
            .unwrap();
        pool.trigger(member).unwrap();
    }
    let mut snapshot = [placeholder(); 3];
    let captured = pool.snapshot_active(&mut snapshot).unwrap();
    assert_eq!(captured, 2);
    assert_eq!([snapshot[0].key, snapshot[1].key], [key(1), key(3)]);

    let joined = pool.admit(key(2), placement(1), 7).unwrap();
    pool.trigger(joined).unwrap();
    assert_eq!(captured, 2);
    assert_eq!([snapshot[0].key, snapshot[1].key], [key(1), key(3)]);
    assert_eq!(pool.snapshot_active(&mut snapshot).unwrap(), 3);
    assert_eq!(
        [snapshot[0].key, snapshot[1].key, snapshot[2].key],
        [key(1), key(2), key(3)]
    );
}

#[test]
fn sign_exhaustion_never_partially_mutates_membership() {
    let mut pool = FixedSharedPool::<1, 1>::new(PoolId(4), 1, 7, 1).unwrap();
    let preparing = pool.admit(key(1), placement(0), 7).unwrap();
    assert_eq!(pool.trigger(preparing), Err(PoolError::SignExhausted));
    assert_eq!(pool.active_population(), 0);
    assert_eq!(pool.population(), 1);
    assert_eq!(pool.signs().count(), 1);
}
