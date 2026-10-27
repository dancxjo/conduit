use super::*;
#[test]
fn actual_snapshot_export_is_signed_private_bounded_and_source_fenced() {
    let (mut store, mut owner, _host, key) = super::super::park_tests::fixture();
    let owner = owner.as_mut().unwrap();
    let original_archives = store.archives.clone();
    checkpoint_store(&mut store, owner, &key).unwrap();
    let generation = store.journal.generation();
    let admissions = serde_json::to_vec(&store.admissions).unwrap();
    let draft = prepare_store(&mut store).unwrap();
    let root = ed25519_dalek::VerifyingKey::from_bytes(&key).unwrap();
    draft
        .offer()
        .verify(&owner.evidence().body_id, &root)
        .unwrap();
    assert_eq!(draft.offer().base.count, 0);
    assert_eq!(draft.offer().end.count, 1);
    let mut bytes = Vec::new();
    export_store(&mut store, &draft, &mut |chunk| {
        bytes.extend_from_slice(chunk);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&bytes)),
        draft.offer().stream.0
    );
    let segments: Vec<BodyBiographyArchiveSegment> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(segments, original_archives);
    assert_eq!(store.journal.generation(), generation);
    assert_eq!(serde_json::to_vec(&store.admissions).unwrap(), admissions);
    // Same exact Body/head but a newer actual snapshot cannot serve an old offer.
    checkpoint_store(&mut store, owner, &key).unwrap();
    let mut emitted = false;
    assert_eq!(
        export_store(&mut store, &draft, &mut |_| {
            emitted = true;
            Ok(())
        }),
        Err("biography-source-changed")
    );
    assert!(!emitted);
}
#[test]
fn interrupted_export_preserves_the_original_producer_custody() {
    let (mut store, mut owner, _host, key) = super::super::park_tests::fixture();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let draft = prepare_store(&mut store).unwrap();
    let generation = store.journal.generation();
    assert!(export_store(&mut store, &draft, &mut |_| Err(Refusal::Storage)).is_err());
    assert_eq!(store.journal.generation(), generation);
    assert!(!store.failed);
    let retry = prepare_store(&mut store).unwrap();
    assert_eq!(
        retry.offer().transcript(&key),
        draft.offer().transcript(&key)
    );
    assert_eq!(store.custody.as_ref().unwrap().local_count, 1);
}

fn endorsed(draft: &Export) -> payload::wire::Receipt {
    let mut receipt = payload::wire::Receipt {
        body: draft.offer.body.clone(),
        summary: draft.offer.end.clone(),
        offer: Hex(draft.offer.transcript(
            &ed25519_dalek::SigningKey::from_bytes(&[7; 32])
                .verifying_key()
                .to_bytes(),
        )),
        consumer: payload::wire::Consumer::Owner { key: Hex([9; 32]) },
        acknowledged: Hex([10; 32]),
        signature: Hex([0; 64]),
    };
    let key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    receipt.signature = Hex(key
        .sign(&receipt.transcript(&key.verifying_key().to_bytes()))
        .to_bytes());
    receipt
}
#[test]
fn cold_format_migration_preserves_body_key_and_original_admissions() {
    let (mut store, mut owner, host, key) = super::super::park_tests::fixture();
    let owner = owner.as_mut().unwrap();
    checkpoint_store(&mut store, owner, &key).unwrap();
    let original = owner.evidence().clone();
    let original_manager = serde_json::to_vec(&store.admissions).unwrap();
    store.evidence = Some(original.clone());
    store.ready = false;
    // Match initialize(): only RAM challenges are retired until ordinary resume.
    store
        .admissions
        .as_mut()
        .unwrap()
        .retire_unfinished_challenges();
    let draft = prepare_store(&mut store).unwrap();
    let receipt = endorsed(&draft);
    let generation = store.journal.generation().unwrap();
    retain_store_receipt(&mut store, receipt.clone()).unwrap();
    assert_eq!(store.journal.generation(), Some(generation + 1));
    assert_eq!(store.seed, Some([7; 32]));
    assert_eq!(store.evidence.as_ref(), Some(&original));
    let custody = store.custody.as_ref().unwrap();
    assert_eq!(custody.local_count, 0);
    assert_eq!(custody.summary.count, receipt.summary.count);
    let mut seed = [0; 32];
    let (resident, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &original.body_id,
        &mut seed,
        None,
    )
    .unwrap();
    assert_eq!(seed, [7; 32]);
    assert_eq!(resident.local_count, 0);
    assert_eq!(resident.retained.evidence, original);
    assert_eq!(
        serde_json::to_vec(&resident.retained.admissions).unwrap(),
        original_manager
    );
    assert!(prepare_store(&mut store).is_err());
    assert!(retain_store_receipt(&mut store, receipt).is_err());
    // Continue through the ordinary checkpoint path without losing the prefix.
    let mut continued =
        SealedBodyLifecycleSession::open_admitted(original, &host.host_id, &host.boot_id).unwrap();
    checkpoint_store(&mut store, &mut continued, &key).unwrap();
    assert_eq!(store.custody.as_ref().unwrap().local_count, 0);
    assert!(store.custody.as_ref().unwrap().prefix.is_some());
}
#[test]
fn foreign_receipt_and_live_owner_cannot_discharge_snapshot() {
    let (mut store, mut owner, _host, key) = super::super::park_tests::fixture();
    let owner = owner.as_mut().unwrap();
    checkpoint_store(&mut store, owner, &key).unwrap();
    let draft = prepare_store(&mut store).unwrap();
    let receipt = endorsed(&draft);
    let generation = store.journal.generation();
    assert!(retain_store_receipt(&mut store, receipt.clone()).is_err());
    store.evidence = Some(owner.evidence().clone());
    store.ready = false;
    for fault in 0..3 {
        let mut invalid = receipt.clone();
        match fault {
            0 => invalid.signature.0[0] ^= 1,
            1 => invalid.offer.0[0] ^= 1,
            _ => invalid.summary.count += 1,
        }
        if fault != 0 {
            let issuer = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
            invalid.signature = Hex(issuer.sign(&invalid.transcript(&key)).to_bytes());
        }
        assert!(retain_store_receipt(&mut store, invalid).is_err());
        assert_eq!(store.journal.generation(), generation);
        assert_eq!(store.custody.as_ref().unwrap().local_count, 1);
    }
}
