//! Actual Store operations on CPU flash; no hardware or execution proof.
use super::*;
fn fixture() -> (
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
    let token = park_store(&mut store, &mut owner, &key, &host.host_id, &host.boot_id).unwrap();
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
    let restored = restore_store(&mut store, token, &host.host_id, &host.boot_id, &key).unwrap();
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
        assert!(restore_store(&mut store, token, &host.host_id, &host.boot_id, &key).is_err());
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
    let token = park_store(&mut store, &mut owner, &key, &host.host_id, &host.boot_id).unwrap();
    let (restored, memory) = allocation_probe::observe(|| {
        restore_store(&mut store, token, &host.host_id, &host.boot_id, &key).unwrap()
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
