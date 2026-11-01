//! Real authenticated CPU transitions, not target preparation qualification.
use super::*;
fn prefix_bytes(
    owner: &SealedBodyLifecycleSession,
    root: &[u8; 32],
    cursor: &payload::network::Cursor,
) -> usize {
    let mut lease = take_store().unwrap();
    let store = lease.0.as_mut().unwrap();
    let previous = store.custody.as_ref().unwrap();
    let prepared =
        prepare_candidate(previous, owner, store.admissions.as_ref(), root, cursor).unwrap();
    prepared
        .measure_frozen(&mut store.journal, &previous.source)
        .unwrap()
        .unwrap()
        .prefix_bytes()
}
#[test]
#[ignore = "owns Boot-global STORE; run this exact test alone in a fresh process"]
fn frozen_pending_clean_reopen_and_allocation_fallback_preserve_exact_custody() {
    for pressure in [false, true] {
        let (mut owner, boot, root, cursor) = setup();
        // First source is the legacy bootstrap format. Ordinary checkpointing
        // migrates its actual archive into authenticated reusable frames.
        with_store(|store| checkpoint_store(store, &mut owner, &root)).unwrap();
        let expected_pending = *cursor.bytes();
        let clean = crate::native_restart::network_recovery::clean_cursor(
            &cursor,
            &SigningKey::from_bytes(&[7; 32]),
        )
        .unwrap();
        let expected_clean = *clean.bytes();
        let prefix = prefix_bytes(&owner, &root, &cursor);
        let encoding_before = payload::rewrite::canonical_encoding_passes();
        let refused =
            pressure.then(|| crate::allocation_probe::refuse_next_allocation_size(prefix));
        let (result, initialization) = crate::allocation_probe::observe(|| {
            CheckpointJob::initialize(take_store().unwrap(), &owner, &boot, &root, cursor)
        });
        drop(refused);
        let mut job = result.unwrap();
        assert_eq!(job.frozen.is_some(), !pressure);
        assert!(!job.effects);
        let cache = job.frozen.as_ref().map_or(0, |f| f.retained_bytes());
        let mut chunks = 0;
        let mut chunk_peak = 0;
        let mut access = Access { fail: false };
        while !job.is_complete() {
            if job.needs_chunk() {
                let (result, memory) =
                    crate::allocation_probe::observe(|| job.prepare_chunk(&owner));
                result.unwrap();
                chunk_peak = chunk_peak.max(memory.peak_bytes);
                chunks += 1;
            }
            job.advance(&mut access).unwrap();
        }
        job.finish(&owner).unwrap();
        assert!(
            job.frozen.is_none(),
            "cache released before canonical finish replay"
        );
        let encodings = payload::rewrite::canonical_encoding_passes() - encoding_before;
        if pressure {
            assert_eq!(encodings, chunks + 3);
        } else {
            assert_eq!(encodings, 3);
        }
        assert_eq!(
            job.lease
                .0
                .as_ref()
                .unwrap()
                .custody
                .as_ref()
                .unwrap()
                .network
                .as_ref()
                .unwrap()
                .bytes(),
            &expected_pending
        );
        // The same retained Store lease executes the actual Clean transition.
        let clean_before = payload::rewrite::canonical_encoding_passes();
        job.begin_clean(&owner, clean).unwrap();
        assert!(job.frozen.is_some());
        while !job.is_complete() {
            if job.needs_chunk() {
                job.prepare_chunk(&owner).unwrap();
            }
            job.advance(&mut access).unwrap();
        }
        job.finish(&owner).unwrap();
        assert_eq!(
            payload::rewrite::canonical_encoding_passes() - clean_before,
            3
        );
        drop(job);
        let store = remove();
        assert!(store.ready && !store.failed);
        let mut seed = PrivateSeed([0; 32]);
        let (resident, _) = decode_checkpoint(
            &mut store.journal,
            &store.binding,
            &owner.evidence().body_id,
            &mut seed.0,
            None,
        )
        .unwrap();
        assert_eq!(resident.network.unwrap().bytes(), &expected_clean);
        assert_eq!(resident.retained.evidence, *owner.evidence());
        println!("network_frozen_cpu: pressure={pressure} pending_encodings={encodings} chunks={chunks} cache_retained_capacities_plus_index={cache} initialize_peak={} initialize_retained={} chunk_peak={chunk_peak} bundled_initial_work=true phase_peak_cpu_only=true target_timing_qualified=false total_target_ram_measured=false",initialization.peak_bytes,initialization.retained_bytes());
    }
}
#[test]
#[ignore = "owns Boot-global STORE; run this exact test alone in a fresh process"]
fn frozen_pending_rejects_owner_change_before_next_program() {
    let (mut owner, boot, root, cursor) = setup();
    // First source is the legacy bootstrap format. Ordinary checkpointing
    // migrates its actual archive into authenticated reusable frames.
    with_store(|store| checkpoint_store(store, &mut owner, &root)).unwrap();
    let mut job =
        CheckpointJob::initialize(take_store().unwrap(), &owner, &boot, &root, cursor).unwrap();
    assert!(job.frozen.is_some());
    let mut access = Access { fail: false };
    while !job.needs_chunk() {
        job.advance(&mut access).unwrap();
    }
    let step = job.step();
    let mut changed = owner.evidence().clone();
    changed.friendly_name.push_str(" changed");
    let changed = SealedBodyLifecycleSession::open(changed).unwrap();
    assert_eq!(
        job.prepare_chunk(&changed),
        Err("network-pending-owner-changed")
    );
    assert_eq!(
        job.step(),
        step,
        "no chunk staged or next program authorized"
    );
    assert!(job.effects, "only prior bank erases have run");
    assert!(job.poisoned);
    drop(job);
    let store = remove();
    assert!(
        store.failed && !store.ready,
        "retain ambiguous prior erase custody"
    );
    assert_eq!(store.journal.generation(), Some(2));
}
