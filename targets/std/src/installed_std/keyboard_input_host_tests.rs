//! Input Host lifetime/ownership mechanisms; no physical input or binding claim.
use super::*;
use conduit_core::HostCallContractId;
use conduit_kernel::{HostCallId, NodeId, RequestId, ValueRef};

struct Pending;
impl crate::hosted_keyboard::HostedKeyboardAdapter for Pending {
    fn poll_next(&mut self) -> crate::hosted_keyboard::HostedKeyboardPoll {
        crate::hosted_keyboard::HostedKeyboardPoll::Pending
    }
}
fn contracts() -> [HostCallContractId; 2] {
    [
        HostCallContractId::from(conduit_std_offers::NEXT_KEY_EVENT_HOST_CALL_CONTRACT),
        HostCallContractId::from(conduit_std_offers::button::NEXT_TRANSITION_HOST_CALL),
    ]
}
fn request(id: u32) -> HostCallRequest {
    HostCallRequest {
        node: NodeId(0),
        request: RequestId(id),
        call: HostCallId(0),
        input: BoundedValueRef::new(
            ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 0,
            },
            0,
        )
        .unwrap(),
    }
}
fn pending(host: &KeyboardInputHost<'_>, kind: InputRequestKind) -> Option<HostCallRequest> {
    match kind {
        InputRequestKind::Keyboard => host.pending_keyboard,
        InputRequestKind::SpaceButton => host.pending_button,
    }
}
#[test]
fn duplicate_or_malformed_request_cannot_replace_owned_input() {
    for kind in [InputRequestKind::Keyboard, InputRequestKind::SpaceButton] {
        let mut adapter = Pending;
        let contracts = contracts();
        let mut host = KeyboardInputHost::new(Some(&mut adapter), &contracts);
        let original = request(0);
        host.accept(original, &[], kind).unwrap();
        assert!(host.accept(request(1), &[1], kind).is_err());
        assert_eq!(pending(&host, kind), Some(original));
        assert!(host.accept(request(1), &[], kind).is_err());
        assert_eq!(pending(&host, kind), Some(original));
    }
}
#[test]
fn cancelled_host_cannot_accept_a_late_request() {
    let mut adapter = Pending;
    let contracts = contracts();
    let mut host = KeyboardInputHost::new(Some(&mut adapter), &contracts);
    host.accept(request(0), &[], InputRequestKind::Keyboard)
        .unwrap();
    host.cancel();
    host.cancel();
    assert!(!host.is_pending());
    for kind in [InputRequestKind::Keyboard, InputRequestKind::SpaceButton] {
        assert!(host.accept(request(1), &[], kind).is_err());
        assert!(pending(&host, kind).is_none());
    }
}
#[test]
fn absent_adapter_never_admits_a_request() {
    let contracts = contracts();
    let mut host = KeyboardInputHost::new(None, &contracts);
    for kind in [InputRequestKind::Keyboard, InputRequestKind::SpaceButton] {
        assert!(host.accept(request(0), &[], kind).is_err());
        assert!(pending(&host, kind).is_none());
    }
}
