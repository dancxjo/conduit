//! CPU transaction and every mocked media-operation cut. Canonical ACK input
//! is proved separately; the writer tests use independently signed DTO fixtures.
use super::*;
use crate::native_body_checkpoint::Storage;
use ed25519_dalek::{Signer, SigningKey};
use std::{cell::RefCell, rc::Rc};
fn digest(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(domain);
    for part in parts {
        h.update(part);
    }
    h.finalize().into()
}
fn candidate(
    body: &conduit_body::BodyId,
    boot: &BootId,
    epoch: u64,
    prior: [u8; 32],
) -> payload::network::Cursor {
    let key = SigningKey::from_bytes(&[7; 32]);
    let member = SigningKey::from_bytes(&[91; 32]);
    let mut bytes = [0; 480];
    bytes[..8].copy_from_slice(b"NCURSOR1");
    bytes[8] = 1;
    bytes[16..24].copy_from_slice(&epoch.to_le_bytes());
    bytes[24..56].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
    bytes[56..88].fill(13);
    bytes[88..96].copy_from_slice(&epoch.to_le_bytes());
    bytes[96..128].copy_from_slice(&Sha256::digest(boot.as_str().as_bytes()));
    bytes[128..132].copy_from_slice(&32768u32.to_le_bytes());
    bytes[132] = 8;
    bytes[136..168].fill(19);
    bytes[200..232].fill(23);
    bytes[232..264].copy_from_slice(member.verifying_key().as_bytes());
    bytes[360..392].copy_from_slice(&prior);
    let offer = digest(
        b"conduit.environment/network-retirement-offer@1\0",
        &[
            key.verifying_key().as_bytes(),
            &bytes[24..168],
            &bytes[360..392],
        ],
    );
    bytes[168..200].copy_from_slice(&offer);
    let ack = digest(
        b"conduit.environment/network-retirement-custody@1\0",
        &[&offer, &bytes[200..232]],
    );
    bytes[264..328].copy_from_slice(&member.sign(&ack).to_bytes());
    bytes[328..360].copy_from_slice(&ack);
    let message = digest(
        b"conduit.environment/network-retirement-receipt@1\0",
        &[&bytes[..24], &bytes[24..408]],
    );
    bytes[408..472].copy_from_slice(&key.sign(&message).to_bytes());
    payload::network::Cursor::decode(bytes).unwrap()
}
fn initial() -> (Store, Box<SealedBodyLifecycleSession>, BootId, [u8; 32]) {
    let (mut store, mut owner, host, root) = super::super::park_tests::fixture();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &root).unwrap();
    (store, owner.unwrap(), host.boot_id, root)
}
#[test]
fn pending_commit_readback_preserves_body_admissions_and_changes_only_receipt_custody() {
    let (mut store, owner, boot, root) = initial();
    let evidence = owner.evidence().clone();
    let admissions = store.admissions.clone();
    let generation = store.journal.generation().unwrap();
    let cursor = candidate(&evidence.body_id, &boot, 0, [0; 32]);
    let expected = *cursor.bytes();
    let committed = persist_authenticated(&mut store, &owner, &boot, &root, cursor).unwrap();
    assert_eq!(committed.generation(), generation + 1);
    assert_eq!(committed.offer(), &expected[168..200]);
    assert!(store.ready && !store.failed);
    assert_eq!(owner.evidence(), &evidence);
    assert_eq!(store.admissions, admissions);
    let mut seed = PrivateSeed([0; 32]);
    let (resident, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &evidence.body_id,
        &mut seed.0,
        None,
    )
    .unwrap();
    assert_eq!(seed.0, [7; 32]);
    assert_eq!(resident.retained.evidence, evidence);
    assert_eq!(resident.retained.admissions, admissions);
    assert_eq!(resident.network.as_ref().unwrap().bytes(), &expected);
    assert_eq!(resident.retained.schema, payload::SCHEMA_V4);
    assert_eq!(resident.summary, store.custody.as_ref().unwrap().summary);
    // A committed duplicate is idempotent and does not rotate either bank.
    let cursor = payload::network::Cursor::decode(expected).unwrap();
    let same = persist_authenticated(&mut store, &owner, &boot, &root, cursor).unwrap();
    assert_eq!(same.generation(), committed.generation());
    let old = store.custody.as_ref().unwrap();
    let next = old.network.as_ref().unwrap();
    assert!(payload::rewrite::PreparedRewrite::prepare_framed_network(
        old.proof.as_ref().unwrap(),
        &old.source,
        &evidence,
        &[],
        admissions.as_ref().unwrap(),
        &old.summary,
        old.span,
        old.prefix.as_ref(),
        old.local_count,
        &root,
        &old.layouts,
        Some(next),
    )
    .is_ok());
    assert!(payload::rewrite::PreparedRewrite::prepare_framed_network(
        old.proof.as_ref().unwrap(),
        &old.source,
        &evidence,
        &[],
        admissions.as_ref().unwrap(),
        &old.summary,
        old.span,
        old.prefix.as_ref(),
        old.local_count,
        &root,
        &old.layouts,
        None,
    )
    .is_err());
}
#[test]
fn pending_refusals_leave_publication_and_original_source_intact() {
    for fault in 0..8 {
        let (mut store, owner, boot, root) = initial();
        let before = store.custody.as_ref().unwrap().source.identity();
        let mut attempted_root = root;
        let mut attempted_boot = boot.clone();
        let mut cursor = candidate(&owner.evidence().body_id, &boot, 0, [0; 32]);
        match fault {
            0 => attempted_root[0] ^= 1,
            1 => attempted_boot = "boot/foreign".into(),
            2 => cursor = candidate(&owner.evidence().body_id, &boot, 1, [0; 32]),
            3 => cursor = candidate(&owner.evidence().body_id, &boot, 0, [1; 32]),
            4 => {
                let mut bytes = *cursor.bytes();
                bytes[408] ^= 1;
                cursor = payload::network::Cursor::decode(bytes).unwrap();
            }
            5 => store.ready = false,
            6 => store.parked = true,
            _ => store.failed = true,
        }
        let ready = store.ready;
        assert!(persist_authenticated(
            &mut store,
            &owner,
            &attempted_boot,
            &attempted_root,
            cursor
        )
        .is_err());
        assert_eq!(
            store.custody.as_ref().unwrap().source.identity() == before,
            true
        );
        assert!(store.custody.as_ref().unwrap().network.is_none());
        assert_eq!(store.ready, ready);
        assert_eq!(store.journal.generation(), Some(1));
    }
}
#[derive(Clone)]
struct Media(Rc<RefCell<Flash>>);
struct Flash {
    bytes: Vec<u8>,
    operations: usize,
    cut: Option<(usize, u8)>,
}
impl Flash {
    fn fault(&mut self) -> Option<u8> {
        self.operations += 1;
        self.cut
            .filter(|(step, _)| *step == self.operations)
            .map(|(_, mode)| mode)
    }
}
impl Storage for Media {
    fn capacity(&self) -> u32 {
        65536
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        let mut f = self.0.borrow_mut();
        if f.fault().is_some() {
            return Err(Refusal::Storage);
        }
        out.copy_from_slice(&f.bytes[at as usize..at as usize + out.len()]);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        let mut f = self.0.borrow_mut();
        let fault = f.fault();
        let n = match fault {
            Some(0) => 0,
            Some(1) => (bytes.len() / 2) & !3,
            _ => bytes.len(),
        };
        for (old, new) in f.bytes[at as usize..at as usize + n]
            .iter_mut()
            .zip(&bytes[..n])
        {
            assert_eq!(*old & new, *new);
            *old &= new;
        }
        if fault.is_some() {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
    fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal> {
        let mut f = self.0.borrow_mut();
        let fault = f.fault();
        let n = match fault {
            Some(0) => 0,
            Some(1) => 4096,
            _ => 32768,
        };
        f.bytes[bank as usize * 32768..bank as usize * 32768 + n].fill(255);
        if fault.is_some() {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
}
fn load<S: Storage>(
    journal: &mut Checkpoint<S>,
    binding: &[u8; 32],
    body: &conduit_body::BodyId,
    root: &[u8; 32],
) -> (ArchiveCustody, Box<payload::archive::Resident>) {
    let mut seed = PrivateSeed([0; 32]);
    let (mut resident, source) = journal
        .load_streamed_with_snapshot(binding, &mut seed.0, |reader| {
            payload::decode_resident_boxed_with_prefix(
                |out| reader.read(out).map_err(|_| ()),
                body,
                root,
                None,
            )
            .map_err(|_| Refusal::Corrupt)
        })
        .unwrap()
        .unwrap();
    assert_eq!(seed.0, [7; 32]);
    payload::authenticate_receipts(&mut resident, &source).unwrap();
    let custody = ArchiveCustody {
        proof: resident.proof.take(),
        layouts: resident.layouts.clone(),
        network: resident
            .network
            .as_ref()
            .map(payload::network::Cursor::try_clone)
            .transpose()
            .unwrap(),
        source,
        summary: resident.summary.clone(),
        span: resident.span,
        prefix: resident.prefix.clone(),
        local_count: resident.local_count,
    };
    (custody, resident)
}
#[test]
fn every_pending_writer_media_cut_reboots_to_exact_old_or_authenticated_pending() {
    let (store, owner, host, root) = super::super::park_tests::fixture();
    let body = &owner.as_ref().unwrap().evidence().body_id;
    let owner = owner.as_ref().unwrap();
    let media = Media(Rc::new(RefCell::new(Flash {
        bytes: vec![255; 65536],
        operations: 0,
        cut: None,
    })));
    let mut journal = Checkpoint::open(media.clone()).unwrap();
    journal
        .save_streamed(&store.binding, &[7; 32], |sink| {
            payload::stream_compressed_with_admissions(
                owner.evidence(),
                &store.archives,
                &[],
                store.admissions.as_ref(),
                sink,
            )
            .map_err(stream_refusal)
        })
        .unwrap();
    let original = media.0.borrow().bytes.clone();
    let cursor = candidate(body, &host.boot_id, 0, [0; 32]);
    let (previous, _) = load(&mut journal, &store.binding, body, &root);
    let prepared = prepare(&previous, owner, store.admissions.as_ref(), &root, &cursor).unwrap();
    media.0.borrow_mut().operations = 0;
    let next =
        write_prepared(&mut journal, &store.binding, &[7; 32], &previous, &prepared).unwrap();
    let operations = media.0.borrow().operations;
    assert_eq!(next.source.generation(), 2);
    // Include all reads, target header/payload/digest/marker programming and
    // target erase. Mutations fail before, halfway through, or after their effect.
    for mode in 0..3 {
        for cut in 1..=operations {
            let media = Media(Rc::new(RefCell::new(Flash {
                bytes: original.clone(),
                operations: 0,
                cut: None,
            })));
            let mut journal = Checkpoint::open(media.clone()).unwrap();
            let (previous, _) = load(&mut journal, &store.binding, body, &root);
            let prepared =
                prepare(&previous, owner, store.admissions.as_ref(), &root, &cursor).unwrap();
            {
                let mut f = media.0.borrow_mut();
                f.operations = 0;
                f.cut = Some((cut, mode));
            }
            assert!(
                write_prepared(&mut journal, &store.binding, &[7; 32], &previous, &prepared)
                    .is_err(),
                "cut {cut}/{mode}"
            );
            // The original bank remains bit-identical for every failed effect.
            assert_eq!(&media.0.borrow().bytes[..32768], &original[..32768]);
            media.0.borrow_mut().cut = None;
            let mut recovered = Checkpoint::open(media).unwrap();
            let (_, resident) = load(&mut recovered, &store.binding, body, &root);
            assert_eq!(resident.retained.evidence, *owner.evidence());
            assert_eq!(resident.retained.admissions, store.admissions);
            assert_eq!(resident.summary, previous.summary);
            if recovered.generation() == Some(1) {
                assert!(resident.network.is_none());
            } else {
                assert_eq!(recovered.generation(), Some(2));
                assert_eq!(resident.network.as_ref().unwrap().bytes(), cursor.bytes());
            }
        }
    }
    println!(
        "Pending writer: {} media-operation cuts across before/partial/after modes",
        operations * 3
    );
}

#[test]
fn committed_ack_read_survives_real_root_new_boot_without_new_snapshot_or_writes() {
    let (mut store, owner, boot, root) = initial();
    let cursor = candidate(&owner.evidence().body_id, &boot, 0, [0; 32]);
    let expected = *cursor.bytes();
    persist_authenticated(&mut store, &owner, &boot, &root, cursor).unwrap();
    let new_boot = BootId::from("boot/root-after-power-cycle");
    let mut returned = SealedBodyLifecycleSession::resume_here(
        owner.evidence().clone(),
        &"host/root".into(),
        &new_boot,
    )
    .unwrap();
    checkpoint_store(&mut store, &mut returned, &root).unwrap();
    let generation = store.journal.generation();
    let read = read_committed(&store, &returned, &root, |bytes| bytes == &expected)
        .unwrap()
        .unwrap();
    assert_eq!(Some(read.generation()), generation);
    assert!(read_committed(&store, &returned, &root, |_| false)
        .unwrap()
        .is_none());
    let cursor = payload::network::Cursor::decode(expected).unwrap();
    let duplicate = persist_authenticated(&mut store, &returned, &new_boot, &root, cursor).unwrap();
    assert_eq!(Some(duplicate.generation()), generation);
    assert_eq!(store.journal.generation(), generation);
}
#[test]
fn next_pending_uses_exact_authenticated_clean_predecessor() {
    let (mut store, owner, host, root) = super::super::park_tests::fixture();
    let owner = owner.unwrap();
    let key = SigningKey::from_bytes(&[7; 32]);
    let clean = payload::network::fixture(&owner.evidence().body_id, &key, 2);
    store
        .journal
        .save_streamed(&store.binding, &[7; 32], |sink| {
            payload::stream_compressed_with_admissions_and_network(
                owner.evidence(),
                &store.archives,
                &[],
                store.admissions.as_ref(),
                Some(&clean),
                sink,
            )
            .map_err(stream_refusal)
        })
        .unwrap();
    let mut seed = PrivateSeed([0; 32]);
    let (r, source) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &owner.evidence().body_id,
        &mut seed.0,
        None,
    )
    .unwrap();
    store.custody = Some(ArchiveCustody {
        proof: r.proof,
        layouts: r.layouts,
        network: r.network,
        source,
        summary: r.summary,
        span: r.span,
        prefix: r.prefix,
        local_count: r.local_count,
    });
    store.archives.clear();
    let (epoch, prior) = clean
        .offer_chain(&owner.evidence().body_id, &key.verifying_key())
        .unwrap();
    let next = candidate(&owner.evidence().body_id, &host.boot_id, epoch, prior);
    let expected = *next.bytes();
    persist_authenticated(&mut store, &owner, &host.boot_id, &root, next).unwrap();
    assert_eq!(
        store
            .custody
            .as_ref()
            .unwrap()
            .network
            .as_ref()
            .unwrap()
            .bytes(),
        &expected
    );
    assert!(
        read_committed(&store, &owner, &root, |bytes| bytes == &expected)
            .unwrap()
            .is_some()
    );
}
