//! Actual Store migration and immutable archive custody, CPU flash only.
use super::super::super::{checkpoint_store, decode_checkpoint, park_tests, Store};
use super::*;
fn bytes(store: &mut Store) -> Vec<u8> {
    let source = &store.custody.as_ref().unwrap().source;
    store
        .journal
        .read_snapshot(source, |r| {
            let mut bytes = Vec::new();
            let mut chunk = [0; 128];
            loop {
                let n = r.read(&mut chunk)?;
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..n]);
            }
            Ok(bytes)
        })
        .unwrap()
}
fn plain(bytes: &[u8]) -> Vec<u8> {
    let mut at = 0;
    let mut result = Vec::new();
    super::super::with_plain(
        |out| {
            let n = 7.min(bytes.len() - at);
            out[..n].copy_from_slice(&bytes[at..at + n]);
            at += n;
            Ok(n)
        },
        |read| {
            let mut chunk = [0; 128];
            loop {
                let n = read(&mut chunk).map_err(|_| "read")?;
                if n == 0 {
                    break;
                }
                result.extend_from_slice(&chunk[..n]);
            }
            Ok(())
        },
    )
    .unwrap();
    result
}
#[test]
fn cold_checkpoint_migrates_once_and_copies_exact_immutable_frames_on_admission_and_append() {
    let (mut store, mut owner, mut host, key) = park_tests::fixture();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let old = bytes(&mut store);
    assert_eq!(old[0], b'B');
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let first = bytes(&mut store);
    assert!(first.starts_with(frames::MAGIC));
    assert_eq!(plain(&old), plain(&first));
    let range = store
        .custody
        .as_ref()
        .unwrap()
        .proof
        .as_ref()
        .unwrap()
        .frames
        .unwrap()
        .0;
    let original = first[range.encoded.start..range.encoded.end].to_vec();
    store
        .admissions
        .as_mut()
        .unwrap()
        .issue_spawn_invitation(
            conduit_body::SpawnInvitationSecret::from_csprng_bytes([34; 32]).unwrap(),
            [35; 32],
            11,
            200,
        )
        .unwrap();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let second = bytes(&mut store);
    let range2 = store
        .custody
        .as_ref()
        .unwrap()
        .proof
        .as_ref()
        .unwrap()
        .frames
        .unwrap()
        .0;
    assert_eq!(&second[range2.encoded.start..range2.encoded.end], original);
    let mut seed = [0; 32];
    let (resident, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &owner.as_ref().unwrap().evidence().body_id,
        &mut seed,
        store.custody.as_ref(),
    )
    .unwrap();
    assert_eq!(
        resident.retained.evidence,
        *owner.as_ref().unwrap().evidence()
    );
    assert_eq!(resident.retained.admissions, store.admissions);
    assert_eq!(
        resident.proof.as_ref().unwrap().frames,
        store
            .custody
            .as_ref()
            .unwrap()
            .proof
            .as_ref()
            .unwrap()
            .frames
    );
    for number in 0..18 {
        host.boot_id = format!("boot/frame-{number}").into();
        let evidence = owner.take().unwrap().evidence().clone();
        owner = Some(Box::new(
            conduit_body::SealedBodyLifecycleSession::resume_here(
                evidence,
                &host.host_id,
                &host.boot_id,
            )
            .unwrap(),
        ));
    }
    owner
        .as_mut()
        .unwrap()
        .seal_membership_history_for_checkpoint()
        .unwrap();
    assert!(!owner.as_ref().unwrap().pending_archives().is_empty());
    let appended = owner.as_ref().unwrap().pending_archives().len() as u64;
    let old_count = store.custody.as_ref().unwrap().local_count;
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let third = bytes(&mut store);
    let range3 = store
        .custody
        .as_ref()
        .unwrap()
        .proof
        .as_ref()
        .unwrap()
        .frames
        .unwrap()
        .0;
    assert!(third[range3.encoded.start..range3.encoded.end].starts_with(&original));
    assert!(range3.encoded.end - range3.encoded.start > original.len());
    assert_eq!(
        store.custody.as_ref().unwrap().local_count,
        old_count + appended
    );
    let (resident, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &owner.as_ref().unwrap().evidence().body_id,
        &mut seed,
        None,
    )
    .unwrap();
    assert_eq!(
        resident.retained.evidence,
        *owner.as_ref().unwrap().evidence()
    );
    println!(
        "frame proof CPU64={} legacy={} framed={} copied={} appended={}",
        core::mem::size_of::<AuthenticatedReceipts>(),
        old.len(),
        first.len(),
        original.len(),
        appended
    );
}
#[test]
fn framed_ingress_refuses_truncation_extra_tags_and_false_archive_boundaries() {
    let (mut store, mut owner, _, key) = park_tests::fixture();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let encoded = bytes(&mut store);
    let body = &owner.as_ref().unwrap().evidence().body_id;
    let decode = |b: &[u8]| {
        let mut at = 0;
        super::super::decode_resident_boxed_with_prefix(
            |out| {
                let n = 1.min(b.len() - at);
                out[..n].copy_from_slice(&b[at..at + n]);
                at += n;
                Ok(n)
            },
            body,
            &key,
            None,
        )
    };
    assert!(decode(&encoded).is_ok());
    for cut in [0, 1, 7, 8, 9, encoded.len() / 2, encoded.len() - 1] {
        assert!(decode(&encoded[..cut]).is_err());
    }
    for extra in [0, 1, 2, 3, b'x'] {
        let mut bad = encoded.clone();
        bad.push(extra);
        assert!(decode(&bad).is_err());
    }
    for (at, value) in [
        (8, frames::ARCHIVE),
        (9, 129),
        (encoded.len() - 1, frames::PREFIX),
    ] {
        let mut bad = encoded.clone();
        bad[at] = value;
        assert!(decode(&bad).is_err());
    }
    // Excess framing alone cannot evade the existing encoded bound: these
    // empty archive frames leave canonical JSON unchanged and retain no index.
    let range = store
        .custody
        .as_ref()
        .unwrap()
        .proof
        .as_ref()
        .unwrap()
        .frames
        .unwrap()
        .0;
    let mut oversized = encoded[..range.encoded.end].to_vec();
    while oversized.len() <= MAX_BYTES {
        frames::encode(frames::ARCHIVE, |_| Ok(()), &mut |b: &[u8]| {
            oversized.extend_from_slice(b);
            Ok::<_, ()>(())
        })
        .unwrap();
    }
    oversized.extend_from_slice(&encoded[range.encoded.end..]);
    assert!(decode(&oversized).is_err());
    let mut at = 0;
    assert!(super::super::plain_length(|out| {
        let n = 128.min(oversized.len() - at);
        out[..n].copy_from_slice(&oversized[at..at + n]);
        at += n;
        Ok(n)
    })
    .is_err());
    let p = plain(&encoded);
    let mut wrong = frames::MAGIC.to_vec();
    for (tag, part) in [
        (frames::PREFIX, &p[..1]),
        (frames::ARCHIVE, &p[1..p.len() - 1]),
        (frames::SUFFIX, &p[p.len() - 1..]),
    ] {
        frames::encode(tag, |sink| sink(part), &mut |b: &[u8]| {
            wrong.extend_from_slice(b);
            Ok::<_, ()>(())
        })
        .unwrap();
    }
    wrong.push(frames::END);
    assert_eq!(plain(&wrong), p);
    assert!(
        decode(&wrong).is_err(),
        "valid JSON with false immutable frame boundaries must refuse"
    );
}

#[test]
#[ignore = "explicit private physical bank+Body input, scalar output only"]
fn private_physical_legacy_migration_preserves_all_history_then_reuses_encoded_archives() {
    use crate::native_body_checkpoint::{Checkpoint, Refusal, Storage};
    let path = std::env::var_os("CONDUIT_PRIVATE_BODY_BACKUP").unwrap();
    let mut backup = std::fs::read(path).unwrap();
    assert!(backup.len() >= 65536);
    let body: conduit_body::BodyId = serde_json::from_value(serde_json::Value::String(
        std::env::var("CONDUIT_PRIVATE_BODY_ID").unwrap(),
    ))
    .unwrap();
    let binding: [u8; 32] = Sha256::digest(body.as_str().as_bytes()).into();
    let mut region = crate::native_evidence_region::test_empty();
    region.program(0, &backup[backup.len() - 65536..]).unwrap();
    backup.fill(0);
    let mut journal = Checkpoint::open(region).unwrap();
    let mut seed = [0; 32];
    let (resident, source) =
        decode_checkpoint(&mut journal, &binding, &body, &mut seed, None).unwrap();
    let root = ed25519_dalek::SigningKey::from_bytes(&seed)
        .verifying_key()
        .to_bytes();
    let archive_digest = |journal: &mut Checkpoint<crate::native_evidence_region::BodyRegion>,
                          source: &crate::native_body_checkpoint::SourceSnapshot,
                          span: archive::Span| {
        let mut digest = Sha256::new();
        journal
            .read_snapshot(source, |r| {
                super::super::export_archive_array(
                    |out| r.read(out).map_err(|_| ()),
                    span,
                    &mut |b: &[u8]| {
                        digest.update(b);
                        Ok::<_, ()>(())
                    },
                )
                .map_err(|_| Refusal::Corrupt)
            })
            .unwrap();
        <[u8; 32]>::from(digest.finalize())
    };
    let original_archives = archive_digest(&mut journal, &source, resident.span);
    let old_length = source.payload_length();
    let old_count = resident.local_count;
    let prepared = PreparedRewrite::prepare_framed(
        resident.proof.as_ref().unwrap(),
        &source,
        &resident.retained.evidence,
        &[],
        resident.retained.admissions.as_ref().unwrap(),
        &resident.summary,
        resident.span,
        resident.prefix.as_ref(),
        resident.local_count,
        &root,
        &resident.layouts,
    )
    .unwrap();
    let mut trial = Vec::new();
    let result = journal.read_snapshot(&source, |r| {
        prepared
            .emit(|out| r.read(out).map_err(|_| ()), &mut |bytes: &[u8]| {
                trial.extend_from_slice(bytes);
                Ok::<_, ()>(())
            })
            .map(|_| ())
            .map_err(|e| match e {
                StreamRefusal::Capacity => Refusal::Capacity,
                _ => Refusal::Corrupt,
            })
    });
    if result.is_err() {
        println!(
            "private_frames preflight_refusal={:?} legacy_encoded={} staged_encoded={}",
            result.err(),
            old_length,
            trial.len()
        );
    }
    assert!(result.is_ok());
    trial.fill(0);
    let started = std::time::Instant::now();
    let completion = journal
        .rewrite_streamed_attested(&source, &binding, &seed, |io| {
            prepared
                .emit(|out| io.read_source(out).map_err(|_| ()), &mut |bytes| {
                    io.write(bytes)
                })
                .map(|_| ())
                .map_err(|_| Refusal::Corrupt)
        })
        .unwrap();
    let migration_us = started.elapsed().as_micros();
    let (next_source, _) = prepared.committed(completion).unwrap();
    let mut checked_seed = [0; 32];
    let (next, checked_source) =
        decode_checkpoint(&mut journal, &binding, &body, &mut checked_seed, None).unwrap();
    assert!(next_source.identity() == checked_source.identity());
    assert!(seed == checked_seed);
    assert!(next.retained.evidence == resident.retained.evidence);
    assert!(next.retained.admissions == resident.retained.admissions);
    assert!(archive_digest(&mut journal, &checked_source, next.span) == original_archives);
    assert!(next.summary == resident.summary);
    assert!(next.layouts == resident.layouts);
    assert!(
        serde_json::to_vec(&next.prefix).unwrap() == serde_json::to_vec(&resident.prefix).unwrap()
    );
    let fast = PreparedRewrite::prepare_framed(
        next.proof.as_ref().unwrap(),
        &checked_source,
        &next.retained.evidence,
        &[],
        next.retained.admissions.as_ref().unwrap(),
        &next.summary,
        next.span,
        next.prefix.as_ref(),
        next.local_count,
        &root,
        &next.layouts,
    )
    .unwrap();
    let started = std::time::Instant::now();
    let mut output = Vec::new();
    journal
        .read_snapshot(&checked_source, |reader| {
            fast.emit(|out| reader.read(out).map_err(|_| ()), &mut |b: &[u8]| {
                output.extend_from_slice(b);
                Ok::<_, ()>(())
            })
            .map_err(|_| Refusal::Corrupt)
        })
        .unwrap();
    let fast_us = started.elapsed().as_micros();
    let stored = journal
        .read_snapshot(&checked_source, |reader| {
            let mut out = Vec::new();
            let mut b = [0; 128];
            loop {
                let n = reader.read(&mut b)?;
                if n == 0 {
                    break;
                }
                out.extend_from_slice(&b[..n]);
            }
            b.fill(0);
            Ok(out)
        })
        .unwrap();
    assert!(stored == output);
    let range = next.proof.as_ref().unwrap().frames.unwrap().0;
    println!("private_frames archives={} legacy_encoded={} framed_encoded={} immutable_encoded={} immutable_plain={} migration_two_pass_us={} reused_one_pass_us={}",old_count,old_length,output.len(),range.encoded.end-range.encoded.start,range.plain.end-range.plain.start,migration_us,fast_us);
    // CPU successor-Boot continuity must reconcile the actual interrupted Wake
    // before canonical invitation issuance; this is not a live Boot claim.
    let observation = next
        .retained
        .evidence
        .membership
        .parts
        .iter()
        .filter_map(|p| p.current.as_ref())
        .find(|o| o.host_id.as_str().starts_with("esp32/"))
        .unwrap();
    let successor_boot: conduit_core::BootId = "boot/private-cpu-successor".into();
    let owner = conduit_body::SealedBodyLifecycleSession::resume_here(
        next.retained.evidence.clone(),
        &observation.host_id,
        &successor_boot,
    )
    .unwrap();
    let mut invited = next.retained.admissions.as_ref().unwrap().clone();
    owner
        .issue_invitation(
            &mut invited,
            conduit_body::SpawnInvitationSecret::from_csprng_bytes([101; 32]).unwrap(),
            [102; 32],
            1,
            2,
            &observation.host_id,
            &successor_boot,
        )
        .unwrap();
    let admission = PreparedRewrite::prepare_framed(
        next.proof.as_ref().unwrap(),
        &checked_source,
        owner.evidence(),
        owner.pending_archives(),
        &invited,
        &next.summary,
        next.span,
        next.prefix.as_ref(),
        next.local_count,
        &root,
        &next.layouts,
    )
    .unwrap();
    let completion = journal
        .rewrite_streamed_attested(&checked_source, &binding, &seed, |io| {
            admission
                .emit(|out| io.read_source(out).map_err(|_| ()), &mut |b| {
                    io.write(b)
                })
                .map(|_| ())
                .map_err(|_| Refusal::Corrupt)
        })
        .unwrap();
    let (admission_source, _) = admission.committed(completion).unwrap();
    let (admitted, _) =
        decode_checkpoint(&mut journal, &binding, &body, &mut checked_seed, None).unwrap();
    assert!(admitted.retained.admissions.as_ref() == Some(&invited));
    assert!(checked_seed == seed);
    assert!(admitted.retained.evidence == *owner.evidence());
    println!(
        "private_frames cpu_successor_boot_invitation_encoded={} remaining_encoded_capacity={}",
        admission_source.payload_length(),
        MAX_BYTES - admission_source.payload_length()
    );
    output.fill(0);
    seed.fill(0);
    checked_seed.fill(0);
}
