use conduit_kernel::shared_flow::{
    FanBranchDisposition, FanError, FixedFan, FixedMerge, MergeError, MergeEvent,
};
use conduit_kernel::shared_pool::{MemberIdentity, MemberKey, MemberPlacement, PoolId};
use conduit_kernel::{Failure, FailureCode, NodeId, ValueRef};

fn member(value: u8, epoch: u32) -> MemberIdentity {
    MemberIdentity {
        pool: PoolId(1),
        key: MemberKey([value; 32]),
        slot: u16::from(value),
        epoch,
        placement: MemberPlacement {
            node: NodeId(u16::from(value)),
            realization: 0,
            play: u16::from(value) + 10,
        },
    }
}

fn value(slot: u16) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: 16,
    }
}

#[test]
fn fan_retains_one_snapshot_and_value_until_every_addressed_branch_terminates() {
    let a = member(1, 1);
    let b = member(2, 1);
    let joined_later = member(3, 1);
    let mut fan = FixedFan::<3>::new().unwrap();
    fan.begin(value(4), &[a, b]).unwrap();

    assert_eq!(fan.next_pending(), Some((a, value(4))));
    fan.observe_full(a).unwrap();
    assert_eq!(fan.next_pending(), Some((a, value(4))));
    fan.deliver(a).unwrap();
    assert_eq!(fan.next_pending(), Some((b, value(4))));
    assert_eq!(fan.deliver(joined_later), Err(FanError::UnknownRecipient));
    assert_eq!(fan.take_terminal_value(), Err(FanError::BranchesPending));

    fan.fail(
        b,
        Failure {
            code: FailureCode::HostOperationFailed,
            detail: 7,
        },
    )
    .unwrap();
    assert_eq!(fan.take_terminal_value().unwrap(), value(4));
}

#[test]
fn fan_preserves_per_branch_delivery_failure_and_cancellation() {
    let recipients = [member(1, 1), member(2, 1), member(3, 1)];
    let mut fan = FixedFan::<3>::new().unwrap();
    fan.begin(value(4), &recipients).unwrap();
    fan.deliver(recipients[0]).unwrap();
    fan.fail(
        recipients[1],
        Failure {
            code: FailureCode::StorageExhausted,
            detail: 2,
        },
    )
    .unwrap();
    fan.cancel(recipients[2]).unwrap();
    let outcomes = fan
        .branches()
        .map(|branch| branch.disposition)
        .collect::<Vec<_>>();
    assert_eq!(outcomes[0], FanBranchDisposition::Delivered);
    assert!(matches!(outcomes[1], FanBranchDisposition::Failed(_)));
    assert_eq!(outcomes[2], FanBranchDisposition::Cancelled);
    assert_eq!(fan.take_terminal_value().unwrap(), value(4));
}

#[test]
fn merge_is_bounded_ordered_and_preserves_exact_source_member_identity() {
    let a = member(1, 1);
    let b = member(2, 4);
    let mut merge = FixedMerge::<2>::new().unwrap();
    merge
        .offer(MergeEvent {
            sequence: 8,
            source: b,
            value: value(8),
        })
        .unwrap();
    merge
        .offer(MergeEvent {
            sequence: 9,
            source: a,
            value: value(9),
        })
        .unwrap();
    assert_eq!(
        merge.offer(MergeEvent {
            sequence: 10,
            source: a,
            value: value(10),
        }),
        Err(MergeError::QueueFull)
    );
    assert_eq!(merge.pop().unwrap().source, b);
    assert_eq!(merge.pop().unwrap().source, a);
    assert!(merge.is_empty());
    assert_eq!(
        merge.offer(MergeEvent {
            sequence: 9,
            source: b,
            value: value(11),
        }),
        Err(MergeError::DuplicateOrOutOfOrderSequence)
    );
}

#[test]
fn fan_rejects_duplicate_recipients_before_retaining_an_item() {
    let a = member(1, 1);
    let mut fan = FixedFan::<2>::new().unwrap();
    assert_eq!(
        fan.begin(value(1), &[a, a]),
        Err(FanError::DuplicateRecipient)
    );
    assert_eq!(fan.value(), None);
}
