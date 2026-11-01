//! Actual cold Store and affine recovery engine on isolated CPU NOR. No hardware.
use super::*;
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
struct Media {
    data: Rc<RefCell<Flash>>,
    identity: [u8; 32],
    capacity: u32,
}
struct Flash {
    bytes: Vec<u8>,
    reads: usize,
    erases: usize,
    operation: usize,
    cut: Option<(usize, u8)>,
    dirty_read: bool,
}
impl Media {
    fn new() -> Self {
        Self {
            data: Rc::new(RefCell::new(Flash {
                // Neighboring partitions are sentinels, never addresses for recovery.
                bytes: (0..40960)
                    .map(|i| {
                        if i < 4096 || i >= 36864 {
                            0x5a
                        } else {
                            (i % 251) as u8
                        }
                    })
                    .collect(),
                reads: 0,
                erases: 0,
                operation: 0,
                cut: None,
                dirty_read: false,
            })),
            identity: [13; 32],
            capacity: CAPACITY,
        }
    }
    fn fault(f: &mut Flash) -> Option<u8> {
        f.operation += 1;
        f.cut
            .filter(|(op, _)| *op == f.operation)
            .map(|(_, mode)| mode)
    }
    fn unchanged_neighbors(&self) {
        let f = self.data.borrow();
        assert!(f.bytes[..4096]
            .iter()
            .chain(f.bytes[36864..].iter())
            .all(|b| *b == 0x5a));
    }
}
impl Storage for Media {
    fn identity(&self) -> [u8; 32] {
        self.identity
    }
    fn capacity(&self) -> u32 {
        self.capacity
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), &'static str> {
        assert!(out.len() <= WORK && at + out.len() as u32 <= CAPACITY);
        let mut f = self.data.borrow_mut();
        f.reads += 1;
        if Self::fault(&mut f).is_some() {
            return Err("mock-read-cut");
        }
        out.copy_from_slice(&f.bytes[4096 + at as usize..4096 + at as usize + out.len()]);
        if f.dirty_read {
            out[0] = 0;
        }
        Ok(())
    }
    fn erase(&mut self, permit: &RecoveryPermit, sector: u8) -> Result<(), &'static str> {
        assert!(permit.permits(self.identity, self.capacity, u32::from(sector) * SECTOR));
        let mut f = self.data.borrow_mut();
        f.erases += 1;
        let mode = Self::fault(&mut f);
        let n = match mode {
            Some(0) => 0,
            Some(1) => 2048,
            _ => 4096,
        };
        let start = 4096 + usize::from(sector) * 4096;
        f.bytes[start..start + n].fill(255);
        if mode.is_some() {
            Err("mock-erase-cut")
        } else {
            Ok(())
        }
    }
}
fn cold() -> Store {
    let (mut store, owner, _, _, _) = super::super::park_tests::network_fixture();
    store.evidence = Some(owner.unwrap().evidence().clone());
    store.ready = false;
    store
}
fn job(store: &Store, media: Media) -> Recovery<Media> {
    match begin_store(store, media).map_err(|(_, e)| e).unwrap() {
        Opened::Recovering(job) => job,
        Opened::Ready(_) => panic!("expected Pending"),
    }
}
fn blank(store: &Store, media: Media) -> Blank<Media> {
    let mut job = job(store, media);
    let mut work = [0; WORK];
    while !job.step(&mut work).unwrap() {}
    job.finish().ok().unwrap()
}
#[test]
fn live_blank_requires_exact_pending_and_adopted_clean_successor() {
    let (mut store, owner, _, root, _) = super::super::park_tests::network_fixture();
    let owner = owner.unwrap();
    let media = Media::new();
    let mut job = begin_live(&store, &owner, &root, media.clone())
        .map_err(|(_, e)| e)
        .unwrap();
    // Live and cold entrances share source checks, but not admission gates.
    assert!(begin_store(&store, media.clone()).is_err());
    let mut work = [0; WORK];
    while !job.complete() {
        job.step_with(&mut work, |storage, permit, step, work| match step {
            Step::Erase(sector) => storage.erase(permit, sector),
            Step::Read(at) => storage.read(at, work),
        })
        .unwrap();
    }
    let blank = job.finish().ok().unwrap();
    check_live_blank(&store, &owner, &root, &blank).unwrap();
    let (blank, error) = finish_live(&store, &owner, &root, blank).err().unwrap();
    assert_eq!(error, "network-live-clean-scope");
    // Exercise the shared authenticated Clean emitter; the production live
    // wrapper uses the incremental writer to adopt this same successor.
    store.ready = false;
    store.evidence = Some(owner.evidence().clone());
    clean_store(&mut store, &blank).unwrap();
    store.evidence = None;
    store.ready = true;
    assert!(check_live_blank(&store, &owner, &root, &blank).is_err());
    let proof = finish_live(&store, &owner, &root, blank)
        .map_err(|(_, e)| e)
        .unwrap();
    let media = proof.into_storage();
    assert!(media.data.borrow().bytes[4096..36864]
        .iter()
        .all(|b| *b == 255));
    media.unchanged_neighbors();
}
#[test]
fn pending_is_affine_until_clean_readback_and_clean_boot_preserves_new_records() {
    let mut store = cold();
    let media = Media::new();
    let old = *store
        .custody
        .as_ref()
        .unwrap()
        .network
        .as_ref()
        .unwrap()
        .bytes();
    let blank = blank(&store, media.clone());
    assert_eq!(media.data.borrow().erases, 8);
    assert_eq!(media.data.borrow().reads, 64);
    clean_store(&mut store, &blank).unwrap();
    let current = store.custody.as_ref().unwrap();
    let clean = current.network.as_ref().unwrap();
    assert_eq!(clean.bytes()[8], 2);
    assert_eq!(&clean.bytes()[24..408], &old[24..408]);
    assert_eq!(&clean.bytes()[16..24], &8u64.to_le_bytes());
    assert_ne!(&clean.bytes()[408..472], &old[408..472]);
    let mut seed = PrivateSeed([0; 32]);
    let (r, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &store.evidence.as_ref().unwrap().body_id,
        &mut seed.0,
        None,
    )
    .unwrap();
    assert_eq!(r.network.as_ref().unwrap().bytes()[8], 2);
    assert_eq!(r.retained.evidence, *store.evidence.as_ref().unwrap());
    assert_eq!(r.retained.admissions, store.admissions);
    assert!(!store.ready && !store.failed);
    // No old Blank may authorize a second Clean transition.
    assert!(clean_store(&mut store, &blank).is_err());
    drop(blank);
    // Emulate an append in the newly opened epoch. Clean Boot performs no I/O.
    media.data.borrow_mut().bytes[4096..4100].copy_from_slice(b"new!");
    let operations = media.data.borrow().operation;
    assert!(matches!(
        begin_store(&store, media.clone()),
        Ok(Opened::Ready(_))
    ));
    assert_eq!(media.data.borrow().operation, operations);
    assert_eq!(&media.data.borrow().bytes[4096..4100], b"new!");
    media.unchanged_neighbors();
}
#[test]
fn no_cursor_never_erases_and_wrong_cold_scope_refuses_without_io() {
    for fault in 0..8 {
        let mut store = cold();
        let mut media = Media::new();
        match fault {
            0 => store.ready = true,
            1 => store.parked = true,
            2 => store.failed = true,
            3 => store.seed = Some([8; 32]),
            4 => store.binding[0] ^= 1,
            5 => media.identity = [14; 32],
            6 => media.capacity = 4096,
            _ => {
                let cursor = store.custody.as_mut().unwrap().network.as_mut().unwrap();
                let mut bytes = *cursor.bytes();
                bytes[408] ^= 1;
                *cursor = payload::network::Cursor::decode(bytes).unwrap();
            }
        }
        let original = media.data.borrow().bytes.clone();
        assert!(begin_store(&store, media.clone()).is_err());
        assert_eq!(media.data.borrow().bytes, original);
        assert_eq!(media.data.borrow().operation, 0);
    }
    let mut store = cold();
    store.custody.as_mut().unwrap().network = None;
    let media = Media::new();
    assert!(matches!(
        begin_store(&store, media.clone()),
        Ok(Opened::Ready(_))
    ));
    assert_eq!(media.data.borrow().operation, 0);
}
#[test]
fn every_network_erase_and_readback_cut_remains_pending_and_resumes_before_clean() {
    for mode in 0..3 {
        for cut in 1..=72 {
            let mut store = cold();
            let media = Media::new();
            let before = store.custody.as_ref().unwrap().source.identity();
            media.data.borrow_mut().cut = Some((cut, mode));
            let mut recovery = job(&store, media.clone());
            let mut work = [0; WORK];
            let mut failed = false;
            for _ in 0..72 {
                if recovery.step(&mut work).is_err() {
                    failed = true;
                    break;
                }
            }
            assert!(failed);
            assert!(recovery.step(&mut work).is_err());
            assert!(recovery.finish().is_err());
            assert!(store.custody.as_ref().unwrap().source.identity() == before);
            assert_eq!(
                store
                    .custody
                    .as_ref()
                    .unwrap()
                    .network
                    .as_ref()
                    .unwrap()
                    .bytes()[8],
                1
            );
            // A new Boot reauthenticates the same Pending; partial NOR erasure
            // does not have to match the original closed-region digest.
            media.data.borrow_mut().cut = None;
            let blank = blank(&store, media.clone());
            clean_store(&mut store, &blank).unwrap();
            assert_eq!(
                store
                    .custody
                    .as_ref()
                    .unwrap()
                    .network
                    .as_ref()
                    .unwrap()
                    .bytes()[8],
                2
            );
            assert!(media.data.borrow().bytes[4096..36864]
                .iter()
                .all(|b| *b == 255));
            media.unchanged_neighbors();
        }
    }
    println!("Recovery:216 media cuts across8 erases and64 full-region readbacks");
}
#[test]
fn dirty_readback_and_stale_blank_cannot_sign_or_commit_clean() {
    let mut store = cold();
    let media = Media::new();
    let mut recovery = job(&store, media.clone());
    media.data.borrow_mut().dirty_read = true;
    let mut work = [0; WORK];
    for _ in 0..8 {
        assert!(!recovery.step(&mut work).unwrap());
    }
    assert_eq!(recovery.step(&mut work), Err("network-recovery-not-blank"));
    assert!(recovery.finish().is_err());
    media.data.borrow_mut().dirty_read = false;
    let blank = blank(&store, media.clone());
    store.seed = Some([8; 32]);
    let generation = store.journal.generation();
    assert!(clean_store(&mut store, &blank).is_err());
    assert_eq!(store.journal.generation(), generation);
    store.seed = Some([7; 32]);
    let refusal = crate::allocation_probe::refuse_next_allocation_size(480);
    assert_eq!(
        clean_store(&mut store, &blank),
        Err("restart-network-cursor-memory")
    );
    drop(refusal);
    assert_eq!(store.journal.generation(), generation);
    assert!(!store.failed);
    clean_store(&mut store, &blank).unwrap();
}

#[path = "native_restart_network_clean_cut_tests.rs"]
mod clean_cuts;

#[path = "native_restart_network_epoch_tests.rs"]
mod epochs;

#[test]
fn pending_and_ambiguous_checkpoint_fence_live_append_even_with_old_journal_handle() {
    let mut store = cold();
    assert_eq!(
        append_allowed(&mut store),
        Err("network-evidence-pending-reclaim")
    );
    let media = Media::new();
    let blank = blank(&store, media);
    clean_store(&mut store, &blank).unwrap();
    assert!(append_allowed(&mut store).is_ok());
    store.parked = true;
    assert!(append_allowed(&mut store).is_ok());
    store.parked = false;
    store.failed = true;
    assert_eq!(
        append_allowed(&mut store),
        Err("network-evidence-restart-failed")
    );
    store.failed = false;
    store
        .journal
        .save(&store.binding, &[7; 32], b"another-generation")
        .unwrap();
    assert_eq!(
        append_allowed(&mut store),
        Err("network-evidence-stale-checkpoint")
    );
}
