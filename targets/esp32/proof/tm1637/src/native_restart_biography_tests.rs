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

/// Actual private media content proof only. Receipt signing here is an explicit
/// CPU fixture; it is neither a real host custody ACK nor physical migration.
#[test]
#[ignore = "requires explicit private backup and Body inputs; never prints private material"]
fn private_backup_prefix_migration_preserves_exact_canonical_custody() {
    let path =
        std::env::var_os("CONDUIT_PRIVATE_BODY_BACKUP").expect("private backup path required");
    let body: conduit_body::BodyId = serde_json::from_value(serde_json::Value::String(
        std::env::var("CONDUIT_PRIVATE_BODY_ID").expect("private Body input required"),
    ))
    .expect("Body input encoding");
    let binding: [u8; 32] = Sha256::digest(body.as_str().as_bytes()).into();
    let mut flash = std::fs::read(path).expect("private backup read");
    assert!(
        flash.len() == 4 * 1024 * 1024,
        "expected complete 4 MiB backup"
    );
    let mut region = crate::native_evidence_region::test_empty();
    crate::native_body_checkpoint::Storage::program(&mut region, 0, &flash[0x3f0000..0x400000])
        .expect("copy exact Body banks");
    flash.fill(0);
    let mut journal = Checkpoint::open(region).expect("actual backup journal");
    let before_generation = journal.generation().expect("retained generation");
    let before_bytes = journal
        .payload_length(&binding)
        .expect("retained binding")
        .expect("retained bytes");
    let mut private_seed = PrivateSeed([0; 32]);
    let (resident, source) =
        decode_checkpoint(&mut journal, &binding, &body, &mut private_seed.0, None)
            .expect("actual authenticated backup decode");
    let key = ed25519_dalek::SigningKey::from_bytes(&private_seed.0);
    let root = key.verifying_key().to_bytes();
    let evidence = resident.retained.evidence;
    let original_manager = resident.retained.admissions;
    let mut admissions = original_manager.clone();
    if let Some(manager) = admissions.as_mut() {
        manager.retire_unfinished_challenges();
    }
    let mut store = Store {
        journal,
        binding,
        seed: Some(private_seed.0),
        evidence: Some(evidence.clone()),
        archives: Vec::new(),
        custody: Some(ArchiveCustody {
            proof: resident.proof,
            layouts: payload::layout::Layouts::default(),
            network: None,
            source,
            summary: resident.summary,
            prefix: resident.prefix,
            local_count: resident.local_count,
            span: resident.span,
        }),
        admissions,
        parked: false,
        pristine: false,
        failed: false,
        ready: false,
    };
    let draft = prepare_store(&mut store).expect("actual snapshot offer");
    let mut exported = Vec::new();
    export_store(&mut store, &draft, &mut |bytes| {
        exported.extend_from_slice(bytes);
        Ok(())
    })
    .expect("actual archive export");
    let archives: Vec<BodyBiographyArchiveSegment> =
        serde_json::from_slice(&exported).expect("canonical exported archives");
    let mut exported_summary = Summary::default();
    for segment in &archives {
        exported_summary
            .append(&body, segment)
            .expect("actual archive chain");
    }
    assert!(
        archives
            .windows(2)
            .any(|pair| pair[1].records.last().unwrap().sequence
                < pair[0].records.last().unwrap().sequence),
        "private fixture exercises canonical category ordering rather than global record order"
    );
    assert!(
        exported_summary == draft.offer().end,
        "all actual archive ordinals/digests preserved"
    );
    assert!(
        <[u8; 32]>::from(Sha256::digest(&exported)) == draft.offer().stream.0,
        "actual array digest preserved"
    );
    assert!(
        draft.offer().base.count == 0,
        "fixture expects original complete local chain"
    );
    let mut receipt = payload::wire::Receipt {
        body: body.clone(),
        summary: draft.offer().end.clone(),
        offer: Hex(draft.offer().transcript(&root)),
        consumer: payload::wire::Consumer::Owner { key: Hex([9; 32]) },
        acknowledged: Hex([10; 32]),
        signature: Hex([0; 64]),
    };
    receipt.signature = Hex(key.sign(&receipt.transcript(&root)).to_bytes());
    retain_store_receipt(&mut store, receipt.clone()).expect("actual CPU format migration");
    let after_bytes = store
        .journal
        .payload_length(&binding)
        .expect("migrated binding")
        .expect("migrated bytes");
    assert!(
        after_bytes < before_bytes,
        "migration must free actual archive capacity"
    );
    assert!(
        store.journal.generation() == Some(before_generation + 1),
        "one exact migration generation"
    );
    assert!(
        store.seed == Some(private_seed.0),
        "private Root identity unchanged"
    );
    assert!(
        store.evidence.as_ref() == Some(&evidence),
        "exact active evidence retained"
    );
    let mut reopened_seed = PrivateSeed([0; 32]);
    let (reopened, _) = decode_checkpoint(
        &mut store.journal,
        &binding,
        &body,
        &mut reopened_seed.0,
        None,
    )
    .expect("offline migrated restart");
    assert!(reopened_seed.0 == private_seed.0, "offline key continuity");
    assert!(
        reopened.retained.evidence == evidence,
        "offline exact biography continuity"
    );
    assert!(
        reopened.retained.admissions == original_manager,
        "snapshot admission manager is not RAM-retired replacement"
    );
    assert!(
        reopened.local_count == 0 && reopened.summary.count == archives.len() as u64,
        "external prefix/local count distinction"
    );
    assert!(
        reopened
            .prefix
            .as_ref()
            .is_some_and(|p| p.signature == receipt.signature && p.summary == exported_summary),
        "offline exact endorsed chain prefix"
    );
    let current = evidence
        .membership
        .parts
        .iter()
        .find_map(|part| part.current.as_ref())
        .expect("existing admitted producer incarnation");
    let cpu_boot: conduit_core::BootId = "boot/private-biography-migration-cpu".into();
    let mut owner =
        SealedBodyLifecycleSession::resume_here(evidence.clone(), &current.host_id, &cpu_boot)
            .expect("canonical new Boot recovery of interrupted Wake");
    let expected_next = owner.evidence().clone();
    let pending_next = owner.pending_archives().len() as u64;
    checkpoint_store(&mut store, &mut owner, &root)
        .expect("ordinary checkpoint preserves external prefix");
    let (continued, _) = decode_checkpoint(
        &mut store.journal,
        &binding,
        &body,
        &mut reopened_seed.0,
        None,
    )
    .expect("ordinary checkpoint offline readback");
    assert!(
        continued.retained.evidence == expected_next && continued.local_count == pending_next,
        "ordinary active evidence/local archive continuity"
    );
    assert!(
        continued
            .prefix
            .as_ref()
            .is_some_and(|p| p.signature == receipt.signature),
        "ordinary checkpoint keeps prefix witness"
    );
    assert!(
        reopened_seed.0 == private_seed.0,
        "ordinary checkpoint keeps key"
    );
    println!("private_backup_migration: generation_before={before_generation} encoded_before={before_bytes} encoded_after={after_bytes} archives={} local_after=0 exact_body_key_evidence_manager=true fixture_receipt_only=true",archives.len());
}

/// Read-only content proof of the actual physical offload. Unlike the earlier
/// CPU migration fixture, this never constructs or signs an endorsement.
#[test]
#[ignore = "requires actual before/after physical backups and private custody files"]
fn private_physical_offload_preserves_actual_receipt_and_complete_archives() {
    let body: conduit_body::BodyId = serde_json::from_value(serde_json::Value::String(
        std::env::var("CONDUIT_PRIVATE_BODY_ID").expect("private Body input required"),
    ))
    .expect("Body input encoding");
    let before_path =
        std::env::var_os("CONDUIT_PRIVATE_BODY_BEFORE").expect("physical before backup required");
    let after_path =
        std::env::var_os("CONDUIT_PRIVATE_BODY_AFTER").expect("physical after backup required");
    let directory = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_PRIVATE_CUSTODY_DIRECTORY")
            .expect("actual private archive files required"),
    );
    let owner_path = std::env::var_os("CONDUIT_PRIVATE_OWNER_PUBLIC")
        .expect("configured owner public key required");
    let binding: [u8; 32] = Sha256::digest(body.as_str().as_bytes()).into();
    let load = |path: std::ffi::OsString| {
        let mut bytes = std::fs::read(path).expect("physical backup read");
        let banks = match bytes.len() {
            0x400000 => &bytes[0x3f0000..0x400000],
            0x10000 => &bytes[..],
            _ => panic!("expected complete flash or exact Body banks"),
        };
        let mut region = crate::native_evidence_region::test_empty();
        crate::native_body_checkpoint::Storage::program(&mut region, 0, banks)
            .expect("copy physical banks");
        bytes.fill(0);
        let mut journal = Checkpoint::open(region).expect("physical bank authentication");
        let generation = journal.generation().expect("physical generation");
        let length = journal
            .payload_length(&binding)
            .expect("physical Body binding")
            .expect("physical payload");
        let mut seed = PrivateSeed([0; 32]);
        let (resident, source) =
            decode_checkpoint(&mut journal, &binding, &body, &mut seed.0, None)
                .expect("physical canonical cold decode");
        (journal, generation, length, seed, resident, source)
    };
    let (before_journal, before_generation, before_bytes, before_seed, before, source) =
        load(before_path);
    let (_after_journal, after_generation, after_bytes, after_seed, after, _) = load(after_path);
    assert!(
        after_generation == before_generation + 1,
        "compare exact storage-only offload, not later Boot checkpoints"
    );
    assert!(
        before_seed.0 == after_seed.0,
        "physical Root private identity preserved"
    );
    assert!(
        before.retained.evidence == after.retained.evidence,
        "exact active canonical evidence preserved"
    );
    assert!(
        before.retained.admissions == after.retained.admissions,
        "exact original admission manager preserved"
    );
    assert!(
        before.prefix.is_none() && before.local_count > 0,
        "original complete local archive chain"
    );
    assert!(
        after.local_count == 0 && after.summary == before.summary,
        "all original archives became endorsed external prefix"
    );
    assert!(
        after_bytes < before_bytes,
        "actual physical capacity recovered"
    );
    let root = ed25519_dalek::SigningKey::from_bytes(&after_seed.0).verifying_key();
    let receipt = after
        .prefix
        .as_ref()
        .expect("actual persisted physical receipt");
    receipt
        .verify(&body, &root)
        .expect("actual physical Root endorsement");
    assert!(
        receipt.summary == cumulative(&before.summary),
        "actual persisted complete prefix summary"
    );
    let owner = std::fs::read(owner_path).expect("configured public owner key");
    assert!(owner.len() == 32, "owner public key extent");
    assert!(
        matches!(&receipt.consumer,payload::wire::Consumer::Owner{key} if key.0==<[u8;32]>::from(Sha256::digest(&owner))),
        "actual receipt records physical owner role, not admitted membership"
    );
    let mut store = Store {
        journal: before_journal,
        binding,
        seed: Some(before_seed.0),
        evidence: Some(before.retained.evidence),
        archives: Vec::new(),
        custody: Some(ArchiveCustody {
            proof: before.proof,
            layouts: payload::layout::Layouts::default(),
            network: None,
            source,
            summary: before.summary,
            prefix: before.prefix,
            local_count: before.local_count,
            span: before.span,
        }),
        admissions: before.retained.admissions,
        parked: false,
        pristine: false,
        failed: false,
        ready: false,
    };
    let draft = prepare_store(&mut store).expect("recompute exact original physical offer");
    assert!(
        draft.offer().transcript(&root.to_bytes()) == receipt.offer.0,
        "actual receipt names authenticated original snapshot"
    );
    let mut exported = Vec::new();
    export_store(&mut store, &draft, &mut |bytes| {
        exported.extend_from_slice(bytes);
        Ok(())
    })
    .expect("exact original archive stream");
    let archives: Vec<BodyBiographyArchiveSegment> =
        serde_json::from_slice(&exported).expect("actual original canonical archives");
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let manifest: payload::wire::Offer = serde_json::from_slice(
        &std::fs::read(directory.join(format!("offer-{}.json", hex(&receipt.offer.0))))
            .expect("actual saved host manifest"),
    )
    .expect("actual manifest encoding");
    assert!(
        manifest
            .verify(&body, &root)
            .expect("host manifest pinned Root")
            == receipt.offer.0,
        "host retained exact physical offer"
    );
    assert!(
        manifest.stream.0 == <[u8; 32]>::from(Sha256::digest(&exported)),
        "host manifest exact source array digest"
    );
    for segment in &archives {
        let saved: BodyBiographyArchiveSegment = serde_json::from_slice(
            &std::fs::read(directory.join(format!("{}.json", hex(&segment.digest))))
                .expect("actual saved host archive"),
        )
        .expect("host canonical archive encoding");
        saved
            .validate()
            .expect("actual saved archive canonical validation");
        assert!(
            saved == *segment,
            "every host archive matches original physical bytes semantically"
        );
    }
    println!("physical_offload_content: generation_before={before_generation} generation_after={after_generation} encoded_before={before_bytes} encoded_after={after_bytes} archives={} actual_owner_receipt_and_host_files=true exact_body_key_evidence_manager=true",archives.len());
}

#[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
#[test]
fn prefix_offload_preserves_durable_layout_and_exact_persisted_manager() {
    let (mut store, mut owner, _host, key) = super::super::park_tests::fixture();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let evidence = owner.as_ref().unwrap().evidence().clone();
    let manager = serde_json::to_vec(&store.admissions).unwrap();
    store.evidence = Some(evidence.clone());
    store.ready = false;
    let layout = super::super::observed::tests::receipt(&evidence.body_id, &[7; 32], 1);
    super::super::observed::persist(&mut store, layout.clone()).unwrap();
    let draft = prepare_store(&mut store).unwrap();
    retain_store_receipt(&mut store, endorsed(&draft)).unwrap();
    let mut seed = PrivateSeed([0; 32]);
    let (resident, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &evidence.body_id,
        &mut seed.0,
        store.custody.as_ref(),
    )
    .unwrap();
    assert_eq!(resident.retained.schema, payload::SCHEMA_V3);
    assert!(resident.prefix.is_some());
    assert_eq!(resident.local_count, 0);
    assert_eq!(
        resident.layouts.find(&[1; 32]).unwrap().bytes(),
        layout.bytes()
    );
    assert_eq!(resident.retained.evidence, evidence);
    assert_eq!(
        serde_json::to_vec(&resident.retained.admissions).unwrap(),
        manager
    );
}

#[test]
fn biography_custody_rewrite_and_followup_checkpoint_preserve_network_cursor() {
    let (mut store, mut owner, host, root, expected) = super::super::park_tests::network_fixture();
    let owner = owner.as_mut().unwrap();
    checkpoint_store(&mut store, owner, &root).unwrap();
    let original = owner.evidence().clone();
    store.evidence = Some(original.clone());
    store.ready = false;
    let draft = prepare_store(&mut store).unwrap();
    let receipt = endorsed(&draft);
    retain_store_receipt(&mut store, receipt).unwrap();
    assert!(store.custody.as_ref().unwrap().network.as_ref() == Some(&expected));
    let mut seed = [0; 32];
    let (resident, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &original.body_id,
        &mut seed,
        None,
    )
    .unwrap();
    assert!(resident.network.as_ref() == Some(&expected));
    assert_eq!(resident.retained.schema, payload::SCHEMA_V4);
    assert_eq!(resident.local_count, 0);
    let mut continued =
        SealedBodyLifecycleSession::open_admitted(original, &host.host_id, &host.boot_id).unwrap();
    checkpoint_store(&mut store, &mut continued, &root).unwrap();
    assert!(store.custody.as_ref().unwrap().network.as_ref() == Some(&expected));
}
