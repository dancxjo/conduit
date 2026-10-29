//! Actual Store operations on CPU flash; no hardware or execution proof.
use super::*;
pub(super) fn fixture() -> (
    Store,
    Option<Box<SealedBodyLifecycleSession>>,
    conduit_core::HostAdvertisement,
    [u8; 32],
) {
    let host = conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "host/root".into(),
        boot_id: "boot/root".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "fixture".into(),
        bases: alloc::vec![],
        resources: alloc::vec![],
        capabilities: alloc::vec![],
        planner_capabilities: alloc::vec![],
    };
    let sign = conduit_core::bind_sign(&host.host_id, &host.boot_id, None, 1);
    let image =
        conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let owner = conduit_environment_runtime::body_image::open_installed_image_sealed(
        image.body.evidence().clone(),
        &host,
    )
    .unwrap();
    let mut evidence = owner.evidence().clone();
    let archive = evidence.seal_membership_history().unwrap().unwrap();
    let owner =
        SealedBodyLifecycleSession::open_admitted(evidence, &host.host_id, &host.boot_id).unwrap();
    let mut manager = AdmissionManager::new(owner.evidence().body_id.clone()).unwrap();
    manager
        .issue_spawn_invitation(
            conduit_body::SpawnInvitationSecret::from_csprng_bytes([19; 32]).unwrap(),
            [20; 32],
            10,
            100,
        )
        .unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&[7; 32])
        .verifying_key()
        .to_bytes();
    let binding = Sha256::digest(owner.evidence().body_id.as_str().as_bytes()).into();
    let store = Store {
        journal: Checkpoint::open(crate::native_evidence_region::test_empty()).unwrap(),
        binding,
        seed: Some([7; 32]),
        evidence: None,
        archives: alloc::vec![archive],
        custody: None,
        admissions: Some(manager),
        parked: false,
        pristine: false,
        failed: false,
        ready: true,
    };
    (store, Some(Box::new(owner)), host, key)
}
#[test]
fn park_restores_exact_same_boot_evidence_archives_and_unretired_admissions() {
    let (mut store, mut owner, host, key) = fixture();
    let foreground = owner.as_ref().unwrap().evidence().body.workset.plots()[1].clone();
    owner.as_mut().unwrap().select_plot(&foreground).unwrap();
    let evidence = owner.as_ref().unwrap().evidence().clone();
    let archives = store.archives.clone();
    let manager = serde_json::to_vec(store.admissions.as_ref().unwrap()).unwrap();
    let mut token = park_store(&mut store, &mut owner, &key, &host.host_id, &host.boot_id).unwrap();
    assert!(owner.is_none());
    assert!(store.parked);
    assert!(!store.ready);
    assert_eq!(store.archives.capacity(), 0);
    assert!(store.admissions.is_none());
    assert!(store.evidence.is_none());
    let generation = store.journal.generation();
    let mut unrelated =
        SealedBodyLifecycleSession::open_admitted(evidence.clone(), &host.host_id, &host.boot_id)
            .unwrap();
    assert!(checkpoint_store(&mut store, &mut unrelated, &key).is_err());
    let restored =
        restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &key).unwrap();
    assert_eq!(restored.evidence(), &evidence);
    assert_eq!(restored.foreground(), Some(&foreground));
    assert!(restored.pending_archives().is_empty());
    assert!(restored.realization().is_none());
    assert!(store.archives.is_empty());
    let custody = store.custody.as_ref().unwrap();
    assert_eq!(custody.summary.count, archives.len() as u64);
    let loaded = store
        .journal
        .read_snapshot(&custody.source, |reader| {
            payload::decode_streamed(|out| reader.read(out).map_err(|_| ()), &evidence.body_id)
                .map_err(|_| Refusal::Corrupt)
        })
        .unwrap();
    assert_eq!(loaded.archives, archives);
    assert_eq!(
        serde_json::to_vec(store.admissions.as_ref().unwrap()).unwrap(),
        manager
    );
    assert_eq!(store.journal.generation(), generation);
    assert!(!store.parked);
    assert!(store.ready);
}
#[test]
fn parked_token_rejects_generation_binding_key_and_current_host_boot_changes() {
    for fault in 0..6 {
        let (mut store, mut owner, host, key) = fixture();
        let mut token =
            park_store(&mut store, &mut owner, &key, &host.host_id, &host.boot_id).unwrap();
        match fault {
            0 => token.generation += 1,
            1 => token.binding[0] ^= 1,
            2 => token.key[0] ^= 1,
            3 => token.host = "foreign-host".into(),
            4 => token.boot = "foreign-boot".into(),
            _ => token.body_id = serde_json::from_str("\"foreign-body\"").unwrap(),
        }
        let generation = store.journal.generation();
        assert!(restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &key).is_err());
        assert!(store.parked);
        assert!(!store.ready);
        assert!(store.admissions.is_none());
        assert_eq!(store.journal.generation(), generation);
    }
}
#[test]
fn park_refusal_preserves_installed_owner_and_all_custody() {
    let (mut store, mut owner, host, key) = fixture();
    let before = owner.as_ref().unwrap().evidence().clone();
    let manager = serde_json::to_vec(store.admissions.as_ref().unwrap()).unwrap();
    assert!(park_store(
        &mut store,
        &mut owner,
        &key,
        &host.host_id,
        &"foreign-boot".into()
    )
    .is_err());
    assert_eq!(owner.as_ref().unwrap().evidence(), &before);
    assert!(store.ready);
    assert!(!store.parked);
    assert_eq!(
        serde_json::to_vec(store.admissions.as_ref().unwrap()).unwrap(),
        manager
    );
    assert!(store.journal.generation().is_none());
}

#[test]
fn retained_restore_keeps_actual_token_and_context_through_early_and_decoded_refusals() {
    let (mut store, mut owner, host, key) = fixture();
    let evidence = owner.as_ref().unwrap().evidence().clone();
    let mut token =
        Some(park_store(&mut store, &mut owner, &key, &host.host_id, &host.boot_id).unwrap());
    let address = token.as_ref().unwrap() as *const ParkedBody;
    let context = token.as_ref().unwrap().proposal.as_ref().unwrap()
        as *const conduit_body::VerifiedBodyProposalContext;
    let generation = store.journal.generation();
    for fault in 0..4 {
        let mut attempted_host = host.host_id.clone();
        let mut attempted_boot = host.boot_id.clone();
        let mut attempted_key = key;
        match fault {
            0 => attempted_host = "foreign-host".into(),
            1 => attempted_boot = "foreign-boot".into(),
            2 => attempted_key[0] ^= 1,
            // Pass scope checks and authenticated decoding, then reject an
            // inconsistent retained private key before publishing any Store.
            _ => store.seed = Some([8; 32]),
        }
        let result = restore_store_retained(
            &mut store,
            &mut token,
            &attempted_host,
            &attempted_boot,
            &attempted_key,
        );
        assert_eq!(
            result.err(),
            Some(if fault == 3 {
                "restart-restore-key"
            } else {
                "restart-restore-scope"
            })
        );
        assert_eq!(token.as_ref().unwrap() as *const ParkedBody, address);
        assert_eq!(
            token.as_ref().unwrap().proposal.as_ref().unwrap()
                as *const conduit_body::VerifiedBodyProposalContext,
            context
        );
        assert_eq!(store.journal.generation(), generation);
        assert!(store.parked);
        assert!(!store.ready);
        assert!(store.admissions.is_none());
        assert!(owner.is_none());
        store.seed = Some([7; 32]);
    }
    let restored =
        restore_store_retained(&mut store, &mut token, &host.host_id, &host.boot_id, &key).unwrap();
    assert!(token.is_none());
    assert_eq!(restored.evidence(), &evidence);
    assert_eq!(store.journal.generation(), generation);
    assert!(!store.parked);
    assert!(store.ready);
    assert!(store.admissions.is_some());
    assert_eq!(
        restore_store_retained(&mut store, &mut token, &host.host_id, &host.boot_id, &key,).err(),
        Some("restart-parked-missing")
    );
    assert!(store.ready);
    assert_eq!(restored.evidence(), &evidence);
}

#[test]
fn archived_boot_history_stays_exact_on_flash_and_out_of_cold_resident_memory() {
    use crate::allocation_probe;
    let (mut store, mut owner, mut host, key) = fixture();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let first_archives = {
        let custody = store.custody.as_ref().unwrap();
        store
            .journal
            .read_snapshot(&custody.source, |reader| {
                payload::decode_streamed(
                    |out| reader.read(out).map_err(|_| ()),
                    &owner.as_ref().unwrap().evidence().body_id,
                )
                .map_err(|_| Refusal::Corrupt)
            })
            .unwrap()
            .archives
    };
    for number in 0..18 {
        host.boot_id = format!("boot/archive-resident-{number:032x}").into();
        let evidence = owner.take().unwrap().evidence().clone();
        owner = Some(Box::new(
            SealedBodyLifecycleSession::resume_here(evidence, &host.host_id, &host.boot_id)
                .unwrap(),
        ));
    }
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    owner
        .as_mut()
        .unwrap()
        .seal_membership_history_for_checkpoint()
        .unwrap();
    let archived = owner.as_ref().unwrap().pending_archives()[0].clone();
    let active = owner.as_ref().unwrap().evidence().clone();
    let ((), memory) = allocation_probe::observe(|| {
        checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap()
    });
    println!(
        "archive rewrite transient peak={} retained={}",
        memory.peak_bytes,
        memory.retained_bytes()
    );
    assert!(
        memory.peak_bytes < 8192,
        "checkpoint must not clone/redecode historical archive: {memory:?}"
    );
    assert!(store.archives.is_empty());
    assert!(owner.as_ref().unwrap().pending_archives().is_empty());
    let custody = store.custody.as_ref().unwrap();
    assert_eq!(custody.summary.count, 2);
    let durable = store
        .journal
        .read_snapshot(&custody.source, |reader| {
            payload::decode_streamed(|out| reader.read(out).map_err(|_| ()), &active.body_id)
                .map_err(|_| Refusal::Corrupt)
        })
        .unwrap();
    assert_eq!(durable.evidence, active);
    assert_eq!(durable.archives, [first_archives[0].clone(), archived]);
    drop(durable);
    let mut token = park_store(&mut store, &mut owner, &key, &host.host_id, &host.boot_id).unwrap();
    let (restored, memory) = allocation_probe::observe(|| {
        restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &key).unwrap()
    });
    println!(
        "custodied cold restore peak={} retained={}",
        memory.peak_bytes,
        memory.retained_bytes()
    );
    assert!(
        memory.peak_bytes < 16384,
        "old archives must not be decoded into resident custody: {memory:?}"
    );
    assert_eq!(restored.evidence(), &active);
    assert!(store.archives.is_empty());
    // The initial boot path still checks every archive object and head in order.
    let custody = store.custody.as_ref().unwrap();
    let checked = store
        .journal
        .read_snapshot(&custody.source, |reader| {
            payload::decode_resident(
                |out| reader.read(out).map_err(|_| ()),
                &active.body_id,
                None,
            )
            .map_err(|_| Refusal::Corrupt)
        })
        .unwrap();
    assert_eq!(checked.summary, custody.summary);
    assert_eq!(checked.span, custody.span);
    assert_eq!(checked.retained.evidence, active);
}

pub(super) fn network_fixture() -> (
    Store,
    Option<Box<SealedBodyLifecycleSession>>,
    conduit_core::HostAdvertisement,
    [u8; 32],
    payload::network::Cursor,
) {
    let (mut store, owner, host, root) = fixture();
    let key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let expected = payload::network::fixture(&owner.as_ref().unwrap().evidence().body_id, &key, 1);
    store
        .journal
        .save_streamed(&store.binding, &[7; 32], |sink| {
            payload::stream_compressed_with_admissions_and_network(
                owner.as_ref().unwrap().evidence(),
                &store.archives,
                &[],
                store.admissions.as_ref(),
                Some(&expected),
                sink,
            )
            .map_err(stream_refusal)
        })
        .unwrap();
    let mut seed = PrivateSeed([0; 32]);
    let body = owner.as_ref().unwrap().evidence().body_id.clone();
    let (resident, source) =
        decode_checkpoint(&mut store.journal, &store.binding, &body, &mut seed.0, None).unwrap();
    store.custody = Some(ArchiveCustody {
        proof: resident.proof,
        layouts: resident.layouts,
        network: resident.network,
        source,
        summary: resident.summary,
        span: resident.span,
        prefix: resident.prefix,
        local_count: resident.local_count,
    });
    store.archives.clear();
    (store, owner, host, root, expected)
}
#[test]
fn network_cursor_survives_actual_checkpoint_framed_rewrite_and_parked_restore() {
    let (mut store, mut owner, host, root, expected) = network_fixture();
    let body = owner.as_ref().unwrap().evidence().body_id.clone();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &root).unwrap();
    assert!(store.custody.as_ref().unwrap().network.as_ref() == Some(&expected));
    // The old wrapper cannot silently drop a cursor using an authenticated proof.
    let previous = store.custody.as_ref().unwrap();
    assert!(payload::rewrite::PreparedRewrite::prepare_framed(
        previous.proof.as_ref().unwrap(),
        &previous.source,
        owner.as_ref().unwrap().evidence(),
        &[],
        store.admissions.as_ref().unwrap(),
        &previous.summary,
        previous.span,
        previous.prefix.as_ref(),
        previous.local_count,
        &root,
        &previous.layouts
    )
    .is_err());
    let changed =
        payload::network::fixture(&body, &ed25519_dalek::SigningKey::from_bytes(&[7; 32]), 2);
    assert!(payload::rewrite::PreparedRewrite::prepare_framed_network(
        previous.proof.as_ref().unwrap(),
        &previous.source,
        owner.as_ref().unwrap().evidence(),
        &[],
        store.admissions.as_ref().unwrap(),
        &previous.summary,
        previous.span,
        previous.prefix.as_ref(),
        previous.local_count,
        &root,
        &previous.layouts,
        Some(&changed)
    )
    .is_err());
    let mut token =
        park_store(&mut store, &mut owner, &root, &host.host_id, &host.boot_id).unwrap();
    let restored =
        restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &root).unwrap();
    assert_eq!(restored.evidence().body_id, body);
    assert!(store.custody.as_ref().unwrap().network.as_ref() == Some(&expected));
}

#[path = "native_restart_admission_storage_tests.rs"]
mod admission_storage_tests;
