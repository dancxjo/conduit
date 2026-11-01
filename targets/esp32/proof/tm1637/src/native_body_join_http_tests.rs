//! Production mailbox regression. State seeding represents already accepted
//! custody; Root admission/flash effects are deliberately outside this proof.
use super::*;
fn proof(nonce: u8) -> Vec<u8> {
    let request = conduit_body::PortableMembershipAdmissionRequest {
        schema: conduit_body::MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
        protocol_version: conduit_core::PROTOCOL_VERSION,
        invitation_id: serde_json::from_str("\"invitation/test\"").unwrap(),
        body_id: serde_json::from_str("\"body/test\"").unwrap(),
        host_id: "host/test".into(),
        boot_id: "boot/test".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        nonce: [nonce; 32],
        signature: [7; 64],
    };
    serde_json::to_vec(&request).unwrap()
}
#[test]
fn retained_custody_remains_accessible_while_publication_is_unavailable() {
    assert!(!crate::native_restart::publication_ready());
    let original = proof(1);
    let other = proof(2);
    // Exercise the actual strict production decoder, not a permissive test stub.
    crate::native_body_join::decode_command(&original).unwrap();
    let digest = Sha256::digest(&original).into();
    STATE.lock(|slot| {
        let mut state = slot.borrow_mut();
        state.enabled = true;
        state.next = 41;
        state.pending = Some(Pending {
            ticket: 41,
            digest,
            bytes: original.clone(),
            membership: true,
            response: None,
        });
    });
    STATE.lock(|slot| {
        let mut state = slot.borrow_mut();
        let original_backing = state.pending.as_ref().unwrap().bytes.as_ptr();
        assert!(take_prepared_pending(&mut state, || Err("body-join-memory")).is_none());
        let retained = state.pending.as_ref().unwrap();
        assert_eq!(retained.ticket, 41);
        assert_eq!(retained.digest, digest);
        assert_eq!(retained.bytes.as_ptr(), original_backing);
        assert_eq!(retained.bytes, original);
        assert!(state.inflight.is_none());
        assert!(state.refused.is_none());
    });
    // A reservation failure restores both original request and the already
    // admitted response allocation; retry performs no replacement allocation.
    let (pending, response) =
        STATE.lock(|slot| take_prepared_pending(&mut slot.borrow_mut(), response_backing).unwrap());
    let request_pointer = pending.bytes.as_ptr();
    let response_pointer = response.as_ptr();
    restore_accepted(Accepted { pending, response });
    let (pending, response) = STATE.lock(|slot| {
        take_prepared_pending(&mut slot.borrow_mut(), || panic!("response reallocated")).unwrap()
    });
    assert_eq!(pending.bytes.as_ptr(), request_pointer);
    assert_eq!(response.as_ptr(), response_pointer);
    let accepted = Accepted { pending, response };
    retained_failure(&accepted, "checkpoint-reconciliation-required");
    assert!(matches!(
        result(41),
        ResultReply::Refused(41, "checkpoint-reconciliation-required")
    ));
    // The same retry remains correlated, while another proof cannot enter.
    assert_eq!(submit(&original), Ok(41));
    assert_eq!(submit(&other), Err("body-join-busy"));
    STATE.lock(|slot| slot.borrow_mut().refused = None);
    restore_accepted(accepted);
    assert_eq!(submit(&original), Ok(41));
    assert!(matches!(result(41), ResultReply::Pending(41)));
    assert_eq!(submit(&other), Err("body-join-busy"));
    // Real service tick observes unavailable storage: it must retain accepted
    // pending input, and never enter a Root admission method.
    crate::native_root::NativeRoot.advance_join(0);
    assert_eq!(submit(&original), Ok(41));
    assert!(matches!(result(41), ResultReply::Pending(41)));
    STATE.lock(|slot| {
        let mut state = slot.borrow_mut();
        let pending = state.pending.take().unwrap();
        assert_eq!(pending.bytes, original);
        state.inflight = Some((pending.ticket, pending.digest));
    });
    assert_eq!(submit(&original), Ok(41));
    assert!(matches!(result(41), ResultReply::Pending(41)));
    assert_eq!(submit(&other), Err("body-join-busy"));
    let receipt = b"retained opaque receipt bytes".to_vec();
    STATE.lock(|slot| {
        let mut state = slot.borrow_mut();
        state.inflight = None;
        state.completed = Some(Completed {
            ticket: 41,
            digest,
            bytes: receipt.clone(),
            deadline: None,
        });
        state.refused = Some((40, "retained-refusal"));
    });
    assert_eq!(submit(&original), Ok(41));
    assert!(matches!(result(41), ResultReply::Complete(bytes) if bytes == receipt));
    assert!(matches!(
        result(40),
        ResultReply::Refused(40, "retained-refusal")
    ));
    STATE.lock(|slot| {
        slot.borrow_mut().pending = Some(Pending {
            ticket: 42,
            digest: Sha256::digest(&other).into(),
            bytes: other.clone(),
            membership: true,
            response: None,
        });
    });
    assert_eq!(submit(&original), Ok(41));
    assert!(matches!(result(41), ResultReply::Complete(bytes) if bytes == receipt));
    assert_eq!(submit(&other), Ok(42));
    STATE.lock(|slot| {
        slot.borrow_mut().pending = None;
    });
    assert_eq!(submit(&other), Err("body-join-unavailable"));
    assert!(matches!(result(42), ResultReply::Unavailable));
    assert!(submit(b"not a proof").is_err());
    assert!(submit(&vec![0; 1025]).is_err());
    // A Root challenge still expires on its own deadline despite retained retry.
    STATE.lock(|slot| slot.borrow_mut().completed.as_mut().unwrap().deadline = Some(0));
    assert_eq!(submit(&original), Ok(41)); // exact deadline remains valid
    crate::manual_clock::Executor::new().advance_millis(1);
    assert_eq!(submit(&original), Err("body-join-unavailable"));
    assert!(matches!(result(41), ResultReply::Unavailable));
    STATE.lock(|slot| assert_eq!(slot.borrow().next, 41));
}
