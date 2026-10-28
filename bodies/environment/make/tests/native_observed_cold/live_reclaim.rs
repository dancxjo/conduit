//! Actual frozen-slot -> prepared custody -> permitted job -> unbound Cold.
//! No SDK, physical timing, current Play, or runtime continuity claim.
use super::*;
fn inspected(body: &conduit_body::BodyId, key: &ed25519_dalek::SigningKey) -> offer::ClosedEpoch {
    let mut inspection = offer::Inspection::start(1, body, &key.verifying_key()).unwrap();
    while !inspection.step().unwrap() {}
    inspection.finish_owned().unwrap()
}
#[test]
fn actual_live_epoch_handoff_refuses_substitution_and_only_rearms_after_permitted_publish() {
    let sign = conduit_core::bind_sign(&"live/host".into(), &"live/boot".into(), None, 0);
    let image =
        conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let body = image.body.evidence().body_id.clone();
    let key = ed25519_dalek::SigningKey::from_bytes(&[73; 32]);
    STATE.lock(|s| *s.borrow_mut() = Some(State::Failed));
    EPOCHS.lock(|e| *e.borrow_mut() = Some(std::array::from_fn(|_| State::Failed)));
    WORK.lock(|w| *w.borrow_mut() = Some(Box::new([0; 536])));
    let media = frozen(1, &body, &key);
    // Simulate the previously durable layout receipt, obtained from the actual
    // journal extent before it is offered by this Boot. Live preparation never
    // signs/persists a replacement receipt or formats unknown metadata.
    let active = EPOCHS.lock(|e| {
        let State::Active(active) =
            core::mem::replace(&mut e.borrow_mut().as_mut().unwrap()[0], State::Failed)
        else {
            panic!("actual active")
        };
        active
    });
    let scope = active.scope;
    let forms = active.forms;
    let (storage, draft) =
        custody_journal::layout::preflight(active.journal, &body, key.verifying_key().as_bytes())
            .ok()
            .unwrap();
    let signature = key.sign(&draft.transcript()).to_bytes();
    let receipt = draft.signed(signature);
    let journal = evidence::Journal::recover(
        storage,
        &scope,
        PREPARED_BASIS.get().unwrap(),
        960,
        &mut [0; 536],
    )
    .unwrap();
    EPOCHS.lock(|e| {
        e.borrow_mut().as_mut().unwrap()[0] = State::Active(state::Active {
            journal,
            scope,
            forms,
            last_sequence: 3,
            failed: false,
            recovered: true,
            signed: true,
            maximum_records: 960,
        })
    });
    let offer = inspected(&body, &key)
        .sign_offer(|draft| {
            let signature = key.sign(&draft.transcript()).to_bytes();
            Ok(draft.signed(signature))
        })
        .unwrap();
    let mut accepted = [0; 416];
    accepted[..32].copy_from_slice(&offer.region.0);
    accepted[32..128].copy_from_slice(&offer.scope.0);
    accepted[128..160].copy_from_slice(&offer.forms.0);
    accepted[160..164].copy_from_slice(&offer.committed.to_le_bytes());
    accepted[164..196].copy_from_slice(&offer.digest.0);
    // This test supplies a genuine Root endorsement. Canonical member/private
    // custody verification is covered by the actual cold Root service tests.
    let sig = key
        .sign(&custody_acceptance::wire::accepted_transcript(&accepted))
        .to_bytes();
    accepted[352..].copy_from_slice(&sig);
    let before = media.lock().unwrap().bytes.clone();
    let mut changed = accepted;
    changed[128] ^= 1;
    let sig = key
        .sign(&custody_acceptance::wire::accepted_transcript(&changed))
        .to_bytes();
    changed[352..].copy_from_slice(&sig);
    assert!(inspected(&body, &key)
        .prepare_reclaim(&changed, &receipt, &body, &key.verifying_key())
        .is_err());
    assert!(offer::Inspection::start(1, &body, &key.verifying_key()).is_ok());
    assert_eq!(media.lock().unwrap().bytes, before);
    let closed = inspected(&body, &key);
    let workspace = WORK.lock(|w| w.borrow_mut().take());
    assert!(closed
        .prepare_reclaim(&accepted, &receipt, &body, &key.verifying_key())
        .is_err());
    WORK.lock(|w| *w.borrow_mut() = workspace);
    assert_eq!(media.lock().unwrap().bytes, before);
    let (destination, prepared) = inspected(&body, &key)
        .prepare_reclaim(&accepted, &receipt, &body, &key.verifying_key())
        .unwrap();
    assert_eq!(
        media.lock().unwrap().bytes,
        before,
        "preparation must write no marker or data"
    );
    EPOCHS.lock(|e| assert!(matches!(&e.borrow().as_ref().unwrap()[0], State::Failed)));
    let job = crate::native_observed_reclaim_job::ReclaimJob::from_prepared(Box::new(
        crate::native_observed_reclaim_job::PreparedStart::new(prepared),
    ));
    let retained = job.into_custody().ok().unwrap();
    destination.retained(retained);
    assert_eq!(
        media.lock().unwrap().bytes,
        before,
        "cancellation cannot write or grant erased custody"
    );
    let storage = EPOCHS.lock(|e| {
        let State::Pending(custody) =
            core::mem::replace(&mut e.borrow_mut().as_mut().unwrap()[0], State::Failed)
        else {
            panic!("same retained owner after cancellation")
        };
        custody.into_storage()
    });
    assert!(Arc::ptr_eq(&storage.media, &media));
    // Reopen the unchanged actual journal, as recovery would; do not construct
    // writable or erased backing from the cancelled preparation.
    let journal = evidence::Journal::recover(
        storage,
        &scope,
        PREPARED_BASIS.get().unwrap(),
        960,
        &mut [0; 536],
    )
    .unwrap();
    assert_eq!(journal.committed_count(), 3);
    EPOCHS.lock(|e| {
        e.borrow_mut().as_mut().unwrap()[0] = State::Active(state::Active {
            journal,
            scope,
            forms,
            last_sequence: 3,
            failed: false,
            recovered: true,
            signed: true,
            maximum_records: 960,
        })
    });
    let (destination, prepared) = inspected(&body, &key)
        .prepare_reclaim(&accepted, &receipt, &body, &key.verifying_key())
        .unwrap();
    let mut job = crate::native_observed_reclaim_job::ReclaimJob::from_prepared(Box::new(
        crate::native_observed_reclaim_job::PreparedStart::new(prepared),
    ));
    let gate = crate::native_flash_window::Gate::new(60_000, 60, 1000).unwrap();
    let wait = gate.begin_wait(0, 60_000).unwrap();
    let mut steps = 0;
    while job.operation().is_some() {
        let permit = wait.try_flash(1, 1000).unwrap();
        job.advance(&permit, &mut [0; 536]).unwrap();
        steps += 1;
        assert!(steps <= 62);
        EPOCHS.lock(|e| assert!(matches!(&e.borrow().as_ref().unwrap()[0], State::Failed)));
    }
    assert_eq!(steps, 62);
    let erased = job.into_erased().ok().unwrap();
    destination.erased(erased);
    EPOCHS.lock(|e| {
        let slots = e.borrow();
        let State::Cold(cold) = &slots.as_ref().unwrap()[0] else {
            panic!("only completed proof creates Cold")
        };
        assert!(cold.scope.is_none());
        assert_eq!(cold.maximum_requested, 0);
    });
    assert_eq!(media.lock().unwrap().erases, 30);
    assert!(media.lock().unwrap().bytes[..custody_journal::DATA]
        .iter()
        .all(|byte| *byte == 255));
}
