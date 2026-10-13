//! Ledger mechanics only. These tests do not mint an owner or runtime input grant.
//! Actual scheduler request completion is exercised by the enclosing runtime path.
use super::*;
use conduit_kernel::{BoundedValueRef, RequestId};

fn request() -> HostCallRequest {
    HostCallRequest {
        node: NodeId(3),
        call: HostCallId(0),
        request: RequestId(0),
        input: BoundedValueRef::new(
            ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 200,
            },
            65_536,
        )
        .unwrap(),
    }
}
fn value() -> ValueRef {
    ValueRef {
        slot: 2,
        generation: 5,
        byte_len: 400,
    }
}
fn recorded() -> InstructionCompletion {
    let mut ledger = InstructionCompletion::new();
    ledger.require_capacity().unwrap();
    ledger.record(request(), value(), [7; 32]);
    ledger
}

#[test]
fn exact_value_origin_requires_node_call_slot_generation_and_length() {
    for (node, call, forged) in [
        (NodeId(2), HostCallId(0), value()),
        (NodeId(3), HostCallId(1), value()),
        (NodeId(3), HostCallId(0), ValueRef { slot: 1, ..value() }),
        (
            NodeId(3),
            HostCallId(0),
            ValueRef {
                generation: 6,
                ..value()
            },
        ),
        (
            NodeId(3),
            HostCallId(0),
            ValueRef {
                byte_len: 399,
                ..value()
            },
        ),
    ] {
        let mut ledger = recorded();
        assert!(ledger.claim(node, call, forged).is_err());
        assert!(ledger.acknowledge(node, call, forged).is_err());
        ledger.claim(NodeId(3), HostCallId(0), value()).unwrap();
        let completed = ledger
            .acknowledge(NodeId(3), HostCallId(0), value())
            .unwrap();
        assert_eq!(completed.request(), request());
        assert_eq!(completed.value(), value());
        ledger.require_transferred(&completed, [7; 32]).unwrap();
    }
}

#[test]
fn fore_ack_transfers_custody_once_without_freeing_source_capacity() {
    let mut ledger = recorded();
    assert!(ledger
        .acknowledge(NodeId(3), HostCallId(0), value())
        .is_err());
    ledger.claim(NodeId(3), HostCallId(0), value()).unwrap();
    assert!(ledger.claim(NodeId(3), HostCallId(0), value()).is_err());
    assert!(ledger.require_capacity().is_err());
    let completed = ledger
        .acknowledge(NodeId(3), HostCallId(0), value())
        .unwrap();
    assert!(ledger
        .acknowledge(NodeId(3), HostCallId(0), value())
        .is_err());
    assert!(
        ledger.require_capacity().is_err(),
        "no later request may replace an outstanding effect origin"
    );
    assert!(ledger.require_transferred(&completed, [8; 32]).is_err());
    ledger.require_transferred(&completed, [7; 32]).unwrap();
    ledger.consume_effect(completed, [7; 32]).unwrap();
    assert!(
        ledger.require_capacity().is_err(),
        "once-per-Mask Source never reopens"
    );
    assert!(ledger.claim(NodeId(3), HostCallId(0), value()).is_err());
}

#[test]
fn revoke_after_fore_ack_invalidates_retained_effect_custody() {
    let mut ledger = recorded();
    ledger.claim(NodeId(3), HostCallId(0), value()).unwrap();
    let completed = ledger
        .acknowledge(NodeId(3), HostCallId(0), value())
        .unwrap();
    ledger.revoke();
    assert!(ledger.require_transferred(&completed, [7; 32]).is_err());
    assert!(ledger.consume_effect(completed, [7; 32]).is_err());
    assert!(ledger.require_capacity().is_err());
}

#[test]
fn absent_and_pre_ack_revoked_ledger_cannot_claim_value() {
    let mut absent = InstructionCompletion::new();
    assert!(absent.claim(NodeId(3), HostCallId(0), value()).is_err());
    let mut ledger = recorded();
    ledger.revoke();
    assert!(ledger.claim(NodeId(3), HostCallId(0), value()).is_err());
    assert!(ledger
        .acknowledge(NodeId(3), HostCallId(0), value())
        .is_err());
}
