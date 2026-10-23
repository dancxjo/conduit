//! Suggested Core owning tests after importing the sibling operation module.
use super::*;
fn policy() -> ProtectedEntryBudget {
    ProtectedEntryBudget {
        accounting: crate::ProtectedEntryAccounting::SourceInterruptsV1,
        quantum_interrupts: 3,
        total_interrupts: 7,
    }
}
#[test]
fn exact_request_keeps_budget_across_gate_and_preempted_slices() {
    let mut owner = ProtectedOperation::new(policy()).unwrap();
    let bytes = [1, 2, 3];
    owner.begin(9_u64, &bytes).unwrap();
    assert_eq!(
        owner.begin(9, &bytes),
        Err(OperationRefusal::DuplicateBegin)
    );
    assert_eq!(owner.grant(&8, &bytes), Err(OperationRefusal::WrongRequest));
    assert_eq!(
        owner.grant(&9, &[1, 2, 4]),
        Err(OperationRefusal::WrongRequest)
    );
    let first = owner.grant(&9, &bytes).unwrap();
    assert_eq!(first.interrupts(), 3);
    assert_eq!(
        owner.grant(&9, &bytes),
        Err(OperationRefusal::SliceOutstanding)
    );
    owner
        .finish_slice(&9, &bytes, first, 2, SliceReturn::Gate)
        .unwrap();
    assert!(!owner.is_terminal());
    assert_eq!(owner.remaining(), 5);
    let next = owner.grant(&9, &bytes).unwrap();
    assert_eq!(
        owner.finish_slice(&9, &bytes, first, 1, SliceReturn::Gate),
        Err(OperationRefusal::StaleSlice)
    );
    owner
        .finish_slice(&9, &bytes, next, 3, SliceReturn::Preempted)
        .unwrap();
    let last = owner.grant(&9, &bytes).unwrap();
    assert_eq!(last.interrupts(), 2);
    assert_eq!(
        owner.finish_slice(&9, &bytes, last, 2, SliceReturn::Preempted),
        Err(OperationRefusal::TotalExhausted)
    );
    assert!(owner.is_terminal());
    assert_eq!(owner.begin(10, &bytes), Err(OperationRefusal::Terminal));
}
#[test]
fn only_full_response_validation_retires_and_foreign_cancel_cannot_revoke() {
    let mut owner = ProtectedOperation::new(policy()).unwrap();
    owner.begin(9_u64, b"command").unwrap();
    assert_eq!(
        owner.cancel_or_fault(&8, b"command"),
        Err(OperationRefusal::WrongRequest)
    );
    let grant = owner.grant(&9, b"command").unwrap();
    assert_eq!(
        owner.complete(&9, b"command", |_, _| panic!("outstanding slice")),
        Err(OperationRefusal::SliceOutstanding)
    );
    owner
        .finish_slice(&9, b"command", grant, 1, SliceReturn::Gate)
        .unwrap();
    assert!(!owner.is_terminal());
    let mut checked = false;
    owner
        .complete(&9, b"command", |request, command| {
            checked = true;
            *request == 9 && command == b"command"
        })
        .unwrap();
    assert!(checked && owner.is_terminal());
    assert_eq!(owner.grant(&9, b"command"), Err(OperationRefusal::Terminal));
}
#[test]
fn invalid_policy_invalid_response_and_invalid_counts_are_closed() {
    let mut invalid = policy();
    invalid.quantum_interrupts = 0;
    assert!(matches!(
        ProtectedOperation::<u64>::new(invalid),
        Err(OperationRefusal::InvalidPolicy)
    ));
    let mut owner = ProtectedOperation::new(policy()).unwrap();
    assert_eq!(owner.grant(&1_u64, b"x"), Err(OperationRefusal::NotBegun));
    owner.begin(1, b"x").unwrap();
    let grant = owner.grant(&1, b"x").unwrap();
    owner
        .finish_slice(&1, b"x", grant, 0, SliceReturn::Gate)
        .unwrap();
    assert_eq!(
        owner.complete(&1, b"x", |_, _| false),
        Err(OperationRefusal::InvalidResponse)
    );
    assert!(owner.is_terminal());
    let mut owner = ProtectedOperation::new(policy()).unwrap();
    owner.begin(1, b"x").unwrap();
    let grant = owner.grant(&1, b"x").unwrap();
    assert_eq!(
        owner.finish_slice(&1, b"x", grant, 4, SliceReturn::Gate),
        Err(OperationRefusal::InvalidCount)
    );
    assert_eq!(owner.remaining(), 0);
    let mut owner = ProtectedOperation::new(policy()).unwrap();
    owner.begin(1, b"x").unwrap();
    owner.cancel_or_fault(&1, b"x").unwrap();
    assert_eq!(owner.grant(&1, b"x"), Err(OperationRefusal::Terminal));
}

#[test]
fn completion_requires_latest_finished_gate_not_unentered_preempted_or_stale_gate() {
    let mut owner = ProtectedOperation::new(policy()).unwrap();
    owner.begin(1_u64, b"x").unwrap();
    assert_eq!(
        owner.complete(&1, b"x", |_, _| panic!("before entry")),
        Err(OperationRefusal::ResponseNotReady)
    );
    let first = owner.grant(&1, b"x").unwrap();
    owner
        .finish_slice(&1, b"x", first, 1, SliceReturn::Preempted)
        .unwrap();
    assert_eq!(
        owner.complete(&1, b"x", |_, _| panic!("preempted")),
        Err(OperationRefusal::ResponseNotReady)
    );
    let gate = owner.grant(&1, b"x").unwrap();
    owner
        .finish_slice(&1, b"x", gate, 1, SliceReturn::Gate)
        .unwrap();
    let resumed = owner.grant(&1, b"x").unwrap();
    assert_eq!(
        owner.complete(&1, b"x", |_, _| panic!("outstanding")),
        Err(OperationRefusal::SliceOutstanding)
    );
    owner
        .finish_slice(&1, b"x", resumed, 1, SliceReturn::Preempted)
        .unwrap();
    assert_eq!(
        owner.complete(&1, b"x", |_, _| panic!("old gate")),
        Err(OperationRefusal::ResponseNotReady)
    );
    assert_eq!(
        owner.finish_slice(&1, b"x", gate, 0, SliceReturn::Gate),
        Err(OperationRefusal::StaleSlice)
    );
    assert_eq!(
        owner.complete(&1, b"x", |_, _| panic!("replayed gate")),
        Err(OperationRefusal::ResponseNotReady)
    );
}

#[test]
fn foreign_grant_cannot_consume_or_enable_the_recipient_operation() {
    for (other_envelope, other_bytes) in [(2_u64, &b"x"[..]), (1, &b"y"[..])] {
        let mut a = ProtectedOperation::new(policy()).unwrap();
        let mut b = ProtectedOperation::new(policy()).unwrap();
        a.begin(other_envelope, other_bytes).unwrap();
        b.begin(1, b"x").unwrap();
        let foreign = a.grant(&other_envelope, other_bytes).unwrap();
        let real = b.grant(&1, b"x").unwrap();
        assert_eq!(foreign.epoch, real.epoch);
        assert_eq!(foreign.interrupts(), real.interrupts());
        assert_eq!(
            b.finish_slice(&1, b"x", foreign, 1, SliceReturn::Gate),
            Err(OperationRefusal::WrongRequest)
        );
        assert_eq!(b.remaining(), policy().total_interrupts);
        assert_eq!(
            b.complete(&1, b"x", |_, _| panic!(
                "foreign grant cannot authorize completion"
            )),
            Err(OperationRefusal::SliceOutstanding)
        );
        b.finish_slice(&1, b"x", real, 1, SliceReturn::Preempted)
            .unwrap();
        assert_eq!(
            b.finish_slice(&1, b"x", foreign, 1, SliceReturn::Gate),
            Err(OperationRefusal::WrongRequest)
        );
        assert_eq!(
            b.complete(&1, b"x", |_, _| panic!(
                "foreign Gate cannot change eligibility"
            )),
            Err(OperationRefusal::ResponseNotReady)
        );
        let real_gate = b.grant(&1, b"x").unwrap();
        b.finish_slice(&1, b"x", real_gate, 1, SliceReturn::Gate)
            .unwrap();
        assert_eq!(
            b.finish_slice(&1, b"x", foreign, 1, SliceReturn::Preempted),
            Err(OperationRefusal::WrongRequest)
        );
        b.complete(&1, b"x", |_, _| true).unwrap();
    }
}
