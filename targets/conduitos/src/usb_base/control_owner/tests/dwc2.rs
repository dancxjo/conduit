//! The real control lease joins the DWC2 packet model, not a replacement grant.
use super::{fixture, *};
use crate::{
    arch::dwc2::{PacketError, test_io},
    usb_base::dwc2_control::*,
};
use alloc::vec::Vec;
fn route() -> RootControlRoute {
    RootControlRoute {
        slot: 1,
        generation: 7,
        address: 0,
        maximum_packet: 8,
        translator: None,
    }
}
fn completed(bytes: Option<&[u8]>) -> (u32, Vec<u32>, Vec<u32>) {
    let Some(bytes) = bytes else {
        return (3, vec![], vec![]);
    };
    let words = bytes
        .chunks(4)
        .map(|chunk| {
            let mut word = [0; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            u32::from_le_bytes(word)
        })
        .collect();
    (3, vec![(2 << 17) | ((bytes.len() as u32) << 4)], words)
}
#[test]
fn dwc2_selected_control_completes_once_without_play_allocations() {
    let owner = fixture::owner(7);
    let node = owner.node;
    let mut controller = test_io::prepared();
    test_io::registers(&mut controller).script.extend([
        completed(None),
        completed(Some(&[9; 8])),
        completed(None),
    ]);
    test_io::registers(&mut controller).writes.reserve(128);
    let mut host = unsafe { Dwc2ControlHost::bind_owned(controller, owner, route()) }.unwrap();
    let request = fixture::input();
    let allocations = crate::test_allocations::allocations(|| {
        let result = host
            .execute(node, HostCallId(0), RequestId(0), &request)
            .unwrap();
        let value = conduit_core::validate_canonical_structured_value(result).unwrap();
        let completed = value.variant_payload("completed").unwrap().unwrap();
        assert_eq!(
            completed
                .record_field("transferred")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u16")
                .unwrap(),
            8_u16.to_le_bytes()
        );
    });
    assert_eq!(allocations, 0);
    assert!(!host.retains_submission());
    assert_eq!(
        host.execute(node, HostCallId(0), RequestId(0), &request),
        Err(Dwc2ControlRefusal::Ownership(
            ControlOwnerRefusal::StaleTransfer
        ))
    );
}
#[test]
fn dwc2_wrong_generation_and_wrong_kernel_binding_never_submit() {
    let owner = fixture::owner(7);
    let mut wrong = route();
    wrong.generation = 8;
    assert!(matches!(
        unsafe { Dwc2ControlHost::bind_owned(test_io::prepared(), owner, wrong) },
        Err(Dwc2ControlRefusal::Binding)
    ));
    let owner = fixture::owner(7);
    let node = owner.node;
    let mut host =
        unsafe { Dwc2ControlHost::bind_owned(test_io::prepared(), owner, route()) }.unwrap();
    assert_eq!(
        host.execute(node, HostCallId(1), RequestId(0), &fixture::input()),
        Err(Dwc2ControlRefusal::Ownership(
            ControlOwnerRefusal::WrongBinding
        ))
    );
    assert!(!host.retains_submission());
}
#[test]
fn dwc2_stall_is_typed_but_nak_retains_lease_until_explicit_retirement() {
    for (status, stalled) in [(2 | (1 << 3), true), (2 | (1 << 4), false)] {
        let owner = fixture::owner(7);
        let node = owner.node;
        let mut controller = test_io::prepared();
        test_io::registers(&mut controller).finish = status;
        let mut host = unsafe { Dwc2ControlHost::bind_owned(controller, owner, route()) }.unwrap();
        let request = fixture::input();
        if stalled {
            let result = host
                .execute(node, HostCallId(0), RequestId(0), &request)
                .unwrap();
            assert!(
                conduit_core::validate_canonical_structured_value(result)
                    .unwrap()
                    .variant_payload("stalled")
                    .unwrap()
                    .is_some()
            );
            assert!(!host.retains_submission());
        } else {
            assert_eq!(
                host.execute(node, HostCallId(0), RequestId(0), &request),
                Err(Dwc2ControlRefusal::Native(PacketError::Nak))
            );
            assert!(host.retains_submission());
            host.revoke(node, HostCallId(0)).unwrap();
            // Revocation does not settle the original physical operation.
            assert!(host.retains_submission());
            host.discard_retired().unwrap();
            assert!(!host.retains_submission());
        }
    }
}
#[test]
fn dwc2_unacknowledged_halt_cannot_retire_by_revoking_software() {
    let owner = fixture::owner(7);
    let node = owner.node;
    let mut controller = test_io::prepared();
    test_io::registers(&mut controller).finish = 0;
    test_io::registers(&mut controller).stuck_halt = true;
    let mut host = unsafe { Dwc2ControlHost::bind_owned(controller, owner, route()) }.unwrap();
    assert_eq!(
        host.execute(node, HostCallId(0), RequestId(0), &fixture::input()),
        Err(Dwc2ControlRefusal::Native(PacketError::Retirement))
    );
    host.revoke(node, HostCallId(0)).unwrap();
    assert_eq!(
        host.discard_retired(),
        Err(Dwc2ControlRefusal::Native(PacketError::Retirement))
    );
    assert!(host.retains_submission());
    match host.release_completed(node, HostCallId(0)) {
        Err((host, reason)) => {
            assert_eq!(reason, Dwc2ControlRefusal::Native(PacketError::Retirement));
            assert!(host.retains_submission());
        }
        Ok(_) => panic!("an unretired controller must remain in custody"),
    }
}

#[test]
fn dwc2_completed_selection_returns_original_controller_for_separately_admitted_use() {
    let owner = fixture::owner(7);
    let node = owner.node;
    let host = unsafe { Dwc2ControlHost::bind_owned(test_io::prepared(), owner, route()) }.unwrap();
    let controller = match host.release_completed(node, HostCallId(0)) {
        Ok(controller) => controller,
        Err(_) => panic!("quiescent selected controller should be returned"),
    };
    assert!(controller.is_prepared());
    assert!(controller.is_quiescent());
}
#[test]
fn dwc2_provider_loss_cannot_implicitly_rebind_a_returning_device() {
    let owner = fixture::owner(7);
    let node = owner.node;
    let mut controller = test_io::prepared();
    test_io::registers(&mut controller).port = 0;
    let mut host = unsafe { Dwc2ControlHost::bind_owned(controller, owner, route()) }.unwrap();
    let request = fixture::input();
    let result = host
        .execute(node, HostCallId(0), RequestId(0), &request)
        .unwrap();
    assert!(
        conduit_core::validate_canonical_structured_value(result)
            .unwrap()
            .variant_payload("provider-lost")
            .unwrap()
            .is_some()
    );
    assert_eq!(
        host.execute(node, HostCallId(0), RequestId(1), &request),
        Err(Dwc2ControlRefusal::Native(PacketError::ProviderLost))
    );
}
