//! Bridge semantics use actual authenticated rewrite/cursor machinery. Canonical
//! admission is separately proved; fixture signatures do not admit a real Host.
use super::*;
use crate::native_body_checkpoint::Storage;
use conduit_core::BootId;
use ed25519_dalek::{Signer, SigningKey};
#[path = "native_restart_network_live_tests.rs"]
mod live;
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

struct Access {
    fail: bool,
}
impl SectorStorage<BodyRegion> for Access {
    // The common CPU BodyRegion interface only exposes whole-bank erase. This
    // bridge stand-in erases once; physical one-sector semantics/cuts are proved
    // by the dedicated incremental writer fixture, not by this adapter.
    fn erase_sector(&mut self, s: &mut BodyRegion, bank: u8, sector: u8) -> Result<(), Refusal> {
        if self.fail {
            return Err(Refusal::Storage);
        }
        if sector == 0 {
            s.erase_bank(bank)?;
        }
        Ok(())
    }
    fn program(&mut self, s: &mut BodyRegion, at: u32, b: &[u8]) -> Result<(), Refusal> {
        if self.fail {
            Err(Refusal::Storage)
        } else {
            s.program(at, b)
        }
    }
    fn read(&mut self, s: &mut BodyRegion, at: u32, b: &mut [u8]) -> Result<(), Refusal> {
        if self.fail {
            Err(Refusal::Storage)
        } else {
            s.read(at, b)
        }
    }
}
fn setup() -> (
    Box<SealedBodyLifecycleSession>,
    BootId,
    [u8; 32],
    payload::network::Cursor,
) {
    let (mut store, mut owner, host, root) = super::super::park_tests::fixture();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &root).unwrap();
    let owner = owner.unwrap();
    let cursor = candidate(&owner.evidence().body_id, &host.boot_id, 0, [0; 32]);
    STORE.lock(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(matches!(*slot, Slot::Uninitialized));
        *slot = Slot::Available(Box::leak(Box::new(store)));
    });
    (owner, host.boot_id, root, cursor)
}
fn remove() -> &'static mut Store {
    STORE.lock(
        |slot| match core::mem::replace(&mut *slot.borrow_mut(), Slot::Uninitialized) {
            Slot::Available(store) => store,
            _ => panic!("missing returned Store"),
        },
    )
}
#[test]
#[ignore = "owns Boot-global STORE; run this exact test alone in a fresh process"]
fn incremental_pending_bridge_custody_cancellation_and_actual_adoption() {
    println!(
        "CheckpointJob={} StoreLease={}",
        core::mem::size_of::<CheckpointJob>(),
        core::mem::size_of::<StoreLease>()
    );
    assert!(reserve_with::<CheckpointJob>(|_| core::ptr::null_mut()).is_err());
    // Complete actual typed Pending checkpoint, with actual exclusive static
    // lease blocking every ordinary Store reader for the entire transaction.
    let (owner, boot, root, cursor) = setup();
    let expected = *cursor.bytes();
    let mut job = Box::new(
        CheckpointJob::initialize(take_store().unwrap(), &owner, &boot, &root, cursor).unwrap(),
    );
    assert!(with_store(|_| Ok(())).is_err());
    let mut access = Access { fail: false };
    loop {
        if job.needs_chunk() {
            job.prepare_chunk(&owner).unwrap();
        }
        if job.advance(&mut access).unwrap() {
            break;
        }
    }
    let committed = job.finish(&owner).unwrap();
    assert_eq!(committed.generation(), 2);
    drop(job);
    let store = remove();
    assert!(store.ready && !store.failed);
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
    let mut seed = PrivateSeed([0; 32]);
    let (resident, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &owner.evidence().body_id,
        &mut seed.0,
        None,
    )
    .unwrap();
    assert_eq!(resident.network.as_ref().unwrap().bytes(), &expected);
    assert_eq!(resident.retained.evidence, *owner.evidence());
    drop(store);
    // Cancellation before effects restores unchanged, live custody.
    let (owner, boot, root, cursor) = setup();
    let job =
        CheckpointJob::initialize(take_store().unwrap(), &owner, &boot, &root, cursor).unwrap();
    drop(job);
    let store = remove();
    assert!(store.ready && !store.failed);
    assert_eq!(store.journal.generation(), Some(1));
    assert!(store.custody.as_ref().unwrap().network.is_none());
    // Cancellation and failed physical operations fence even ambiguous effects.
    for fail in [false, true] {
        let (owner, boot, root, cursor) = setup();
        let mut job =
            CheckpointJob::initialize(take_store().unwrap(), &owner, &boot, &root, cursor).unwrap();
        let result = job.advance(&mut Access { fail });
        assert_eq!(result.is_err(), fail);
        drop(job);
        let store = remove();
        assert!(store.failed && !store.ready);
        assert_eq!(store.journal.generation(), Some(1));
        assert!(store.custody.as_ref().unwrap().network.is_none());
    }
}
