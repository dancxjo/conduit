//! Affine source provenance and two-bank copy-forward storage proof.
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
mod checkpoint;
use checkpoint::{Checkpoint, Refusal, RewriteIo, SourceSnapshot};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Clone)]
struct Flash {
    bytes: Rc<RefCell<Vec<u8>>>,
    stages: Rc<Cell<usize>>,
    cut: Rc<Cell<Option<usize>>>,
    commit_ack_lost: Rc<Cell<bool>>,
}
impl Flash {
    fn new(bytes: Vec<u8>, cut: Option<usize>) -> Self {
        Self {
            bytes: Rc::new(RefCell::new(bytes)),
            stages: Rc::new(Cell::new(0)),
            cut: Rc::new(Cell::new(cut)),
            commit_ack_lost: Rc::new(Cell::new(false)),
        }
    }
    fn stage(&self) -> Result<(), Refusal> {
        let stage = self.stages.get();
        if self.cut.get() == Some(stage) {
            return Err(Refusal::Storage);
        }
        self.stages.set(stage + 1);
        Ok(())
    }
}
impl checkpoint::Storage for Flash {
    fn capacity(&self) -> u32 {
        65536
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        out.copy_from_slice(
            self.bytes
                .borrow()
                .get(at as usize..at as usize + out.len())
                .ok_or(Refusal::Storage)?,
        );
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        assert_eq!(at % 4, 0);
        assert_eq!(bytes.len() % 4, 0);
        for (index, word) in bytes.as_chunks::<4>().0.iter().enumerate() {
            self.stage()?;
            let start = at as usize + index * 4;
            for (old, new) in self.bytes.borrow_mut()[start..start + 4]
                .iter_mut()
                .zip(word)
            {
                assert_eq!(*old & new, *new);
                *old &= *new;
            }
        }
        // The physical COMMIT may have landed even if the driver reports a
        // transport error. The live owner must retain its original source.
        if at % 32768 == 152 && self.commit_ack_lost.replace(false) {
            return Err(Refusal::Storage);
        }
        Ok(())
    }
    fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal> {
        for sector in 0..8 {
            self.stage()?;
            let at = usize::from(bank) * 32768 + sector * 4096;
            self.bytes.borrow_mut()[at..at + 4096].fill(255);
        }
        Ok(())
    }
}
const BINDING: [u8; 32] = [33; 32];
const SEED: [u8; 32] = [34; 32];
fn collect(reader: &mut checkpoint::PayloadReader<'_, Flash>) -> Result<Vec<u8>, Refusal> {
    let mut output = Vec::new();
    let mut chunk = [0; 128];
    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        output.extend_from_slice(&chunk[..n]);
    }
    Ok(output)
}
fn witnessed(journal: &mut Checkpoint<Flash>, expected: &[u8]) -> SourceSnapshot {
    let mut seed = [0; 32];
    let (value, witness) = journal
        .load_streamed_with_snapshot(&BINDING, &mut seed, |reader| {
            let value = collect(reader)?;
            if value != expected {
                return Err(Refusal::Corrupt);
            }
            Ok(value)
        })
        .unwrap()
        .unwrap();
    assert_eq!(value, expected);
    assert_eq!(seed, SEED);
    assert_eq!(witness.binding(), &BINDING);
    assert_eq!(witness.payload_length(), expected.len());
    witness
}
fn copy_forward(io: &RewriteIo<'_, Flash>, prefix: &[u8]) -> Result<(), Refusal> {
    io.write(prefix)?;
    let mut chunk = [0; 128];
    loop {
        let n = io.read_source(&mut chunk)?;
        if n == 0 {
            break;
        }
        io.write(&chunk[..n])?;
    }
    Ok(())
}
fn initial() -> (Flash, Vec<u8>) {
    let plain = (0u8..=255).cycle().take(4099).collect::<Vec<_>>();
    let media = Flash::new(vec![255; 65536], None);
    Checkpoint::open(media.clone())
        .unwrap()
        .save(&BINDING, &SEED, &plain)
        .unwrap();
    media.stages.set(0);
    (media, plain)
}
#[test]
fn pinned_source_rewrite_interleaves_reads_and_writes_and_retires_old_witness() {
    let (media, plain) = initial();
    let mut journal = Checkpoint::open(media).unwrap();
    let old = witnessed(&mut journal, &plain);
    assert_eq!(old.generation(), 1);
    assert_eq!(journal.read_snapshot(&old, collect).unwrap(), plain);
    let new = journal
        .rewrite_streamed(&old, &BINDING, &SEED, |io| copy_forward(io, b"prefix/"))
        .unwrap();
    let mut expected = b"prefix/".to_vec();
    expected.extend_from_slice(&plain);
    assert_eq!(new.generation(), 2);
    assert_eq!(new.payload_length(), expected.len());
    assert_eq!(journal.read_snapshot(&new, collect).unwrap(), expected);
    assert!(matches!(
        journal.read_snapshot(&old, collect),
        Err(Refusal::Generation)
    ));
    assert!(matches!(
        journal.rewrite_streamed(&old, &BINDING, &SEED, |io| copy_forward(io, b"bad")),
        Err(Refusal::Generation)
    ));
}
#[test]
fn no_witness_or_seed_escapes_typed_refusal_or_incomplete_consumption() {
    let (media, plain) = initial();
    let mut journal = Checkpoint::open(media.clone()).unwrap();
    let mut seed = [99; 32];
    assert!(matches!(
        journal.load_streamed_with_snapshot::<()>(&BINDING, &mut seed, |_| Err(Refusal::Corrupt)),
        Err(Refusal::Corrupt)
    ));
    assert_eq!(seed, [0; 32]);
    seed.fill(99);
    assert!(matches!(
        journal.load_streamed_with_snapshot(&BINDING, &mut seed, |reader| {
            reader.read(&mut [0; 128])?;
            Ok(())
        }),
        Err(Refusal::Corrupt)
    ));
    assert_eq!(seed, [0; 32]);
    media.bytes.borrow_mut()[160] ^= 1;
    let called = Cell::new(false);
    assert!(journal
        .load_streamed_with_snapshot(&BINDING, &mut seed, |_| {
            called.set(true);
            Ok(())
        })
        .is_err());
    assert!(!called.get());
    assert_eq!(seed, [0; 32]);
    assert_eq!(plain.len(), 4099);
}
#[test]
fn preflight_failure_does_not_erase_and_differing_second_pass_never_commits() {
    let (media, plain) = initial();
    let original = media.bytes.borrow().clone();
    let mut journal = Checkpoint::open(media.clone()).unwrap();
    let source = witnessed(&mut journal, &plain);
    assert!(journal
        .rewrite_streamed(&source, &BINDING, &SEED, |io| io
            .write(b"without consuming source"))
        .is_err());
    assert_eq!(*media.bytes.borrow(), original);
    assert!(journal
        .rewrite_streamed(&source, &BINDING, &SEED, |io| {
            for _ in 0..256 {
                io.write(&[0; 128])?;
            }
            copy_forward(io, b"")
        })
        .is_err());
    assert_eq!(*media.bytes.borrow(), original);
    assert!(journal
        .rewrite_streamed(&source, &[35; 32], &SEED, |io| copy_forward(io, b""))
        .is_err());
    assert_eq!(*media.bytes.borrow(), original);
    let mut pass = 0;
    assert!(matches!(
        journal.rewrite_streamed(&source, &BINDING, &SEED, |io| {
            pass += 1;
            copy_forward(io, if pass == 1 { b"first" } else { b"other" })
        }),
        Err(Refusal::Corrupt)
    ));
    assert_eq!(journal.generation(), Some(1));
    assert_eq!(journal.read_snapshot(&source, collect).unwrap(), plain);
    let mut reopen = Checkpoint::open(media).unwrap();
    let mut seed = [0; 32];
    let mut out = vec![0; 32608];
    let n = reopen.load(&BINDING, &mut seed, &mut out).unwrap().unwrap();
    assert_eq!(&out[..n], plain);
    assert_eq!(seed, SEED);
}
#[test]
fn changed_source_during_either_pass_refuses_success_and_new_commit() {
    for changed_pass in [1, 2] {
        let (media, plain) = initial();
        let mut journal = Checkpoint::open(media.clone()).unwrap();
        let source = witnessed(&mut journal, &plain);
        let mut pass = 0;
        assert!(matches!(
            journal.rewrite_streamed(&source, &BINDING, &SEED, |io| {
                pass += 1;
                if pass == changed_pass {
                    media.bytes.borrow_mut()[160] ^= 1;
                }
                copy_forward(io, b"new/")
            }),
            Err(Refusal::Corrupt)
        ));
        assert_eq!(&media.bytes.borrow()[32768 + 152..32768 + 156], &[255; 4]);
        assert_eq!(journal.generation(), Some(1));
    }
}
#[test]
fn every_program_word_and_sector_cut_keeps_exact_old_or_new_source_custody() {
    let (media, plain) = initial();
    let original = media.bytes.borrow().clone();
    let mut journal = Checkpoint::open(media.clone()).unwrap();
    let source = witnessed(&mut journal, &plain);
    journal
        .rewrite_streamed(&source, &BINDING, &SEED, |io| copy_forward(io, b"new/"))
        .unwrap();
    let stages = media.stages.get();
    let mut expected = b"new/".to_vec();
    expected.extend_from_slice(&plain);
    for cut in 0..=stages {
        let media = Flash::new(original.clone(), Some(cut));
        let mut journal = Checkpoint::open(media.clone()).unwrap();
        let source = witnessed(&mut journal, &plain);
        let result =
            journal.rewrite_streamed(&source, &BINDING, &SEED, |io| copy_forward(io, b"new/"));
        assert_eq!(result.is_ok(), cut == stages, "cut{cut}/{stages}");
        if result.is_err() {
            assert_eq!(journal.read_snapshot(&source, collect).unwrap(), plain);
        }
        media.cut.set(None);
        if result.is_err() {
            let next = journal
                .rewrite_streamed(&source, &BINDING, &SEED, |io| copy_forward(io, b"new/"))
                .unwrap();
            assert_eq!(next.generation(), 2);
            assert_eq!(journal.read_snapshot(&next, collect).unwrap(), expected);
            assert!(matches!(
                journal.read_snapshot(&source, collect),
                Err(Refusal::Generation)
            ));
        }
        let mut reopened = Checkpoint::open(media).unwrap();
        let mut seed = [0; 32];
        let mut out = vec![0; 32608];
        let n = reopened
            .load(&BINDING, &mut seed, &mut out)
            .unwrap()
            .unwrap();
        out.truncate(n);
        assert!(out == plain || out == expected, "cut{cut}");
        assert_eq!(seed, SEED);
    }
    println!("snapshot_copy_forward_powercut_stages={stages}");
}

#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn authenticated_copy_forward_keeps_only_fixed_storage_state() {
    let (media, plain) = initial();
    let mut journal = Checkpoint::open(media).unwrap();
    let source = witnessed(&mut journal, &plain);
    let (next, memory) = allocation_probe::observe(|| {
        journal
            .rewrite_streamed(&source, &BINDING, &SEED, |io| copy_forward(io, b"new/"))
            .unwrap()
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(next.generation(), 2);
}

#[test]
fn physically_committed_but_unacknowledged_successor_keeps_live_source_retryable() {
    let (media, plain) = initial();
    let mut journal = Checkpoint::open(media.clone()).unwrap();
    let source = witnessed(&mut journal, &plain);
    media.commit_ack_lost.set(true);
    assert!(matches!(
        journal.rewrite_streamed(&source, &BINDING, &SEED, |io| copy_forward(io, b"new/")),
        Err(Refusal::Storage)
    ));
    assert_eq!(journal.generation(), Some(1));
    assert_eq!(journal.read_snapshot(&source, collect).unwrap(), plain);
    let mut expected = b"new/".to_vec();
    expected.extend_from_slice(&plain);
    // A separate reboot observer already sees generation 2, but no live
    // acknowledgement escaped the original owner's failed operation.
    let mut reboot = Checkpoint::open(media.clone()).unwrap();
    assert_eq!(reboot.generation(), Some(2));
    let reboot_source = witnessed(&mut reboot, &expected);
    assert_eq!(
        reboot.read_snapshot(&reboot_source, collect).unwrap(),
        expected
    );
    let retry = journal
        .rewrite_streamed(&source, &BINDING, &SEED, |io| copy_forward(io, b"new/"))
        .unwrap();
    assert_eq!(retry.generation(), 2);
    assert_eq!(journal.read_snapshot(&retry, collect).unwrap(), expected);
}
