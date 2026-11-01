use super::*;
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
struct Media(Rc<RefCell<Vec<u8>>>);
impl Storage for Media {
    fn capacity(&self) -> u32 {
        65536
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        out.copy_from_slice(&self.0.borrow()[at as usize..at as usize + out.len()]);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        for (old, new) in self.0.borrow_mut()[at as usize..at as usize + bytes.len()]
            .iter_mut()
            .zip(bytes)
        {
            assert_eq!(*old & *new, *new);
            *old &= *new;
        }
        Ok(())
    }
    fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal> {
        self.0.borrow_mut()[bank as usize * 32768..(bank as usize + 1) * 32768].fill(255);
        Ok(())
    }
}
struct Access {
    calls: usize,
    cut: Option<(usize, u8)>,
}
impl Access {
    fn fault(&mut self) -> Option<u8> {
        self.calls += 1;
        self.cut.filter(|(n, _)| *n == self.calls).map(|(_, m)| m)
    }
}
impl SectorStorage<Media> for Access {
    fn erase_sector(&mut self, s: &mut Media, bank: u8, sector: u8) -> Result<(), Refusal> {
        let f = self.fault();
        let n = match f {
            Some(0) => 0,
            Some(1) => 2048,
            _ => 4096,
        };
        let at = bank as usize * 32768 + sector as usize * 4096;
        s.0.borrow_mut()[at..at + n].fill(255);
        if f.is_some() {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
    fn program(&mut self, s: &mut Media, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        let f = self.fault();
        let n = match f {
            Some(0) => 0,
            Some(1) => (bytes.len() / 2) & !3,
            _ => bytes.len(),
        };
        s.program(at, &bytes[..n])?;
        if f.is_some() {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
    fn read(&mut self, s: &mut Media, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        if self.fault().is_some() {
            Err(Refusal::Storage)
        } else {
            s.read(at, out)
        }
    }
}
fn emit(
    reader: &mut PayloadReader<'_, Media>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    let mut block = PrivateChunk([0; 128]);
    loop {
        let n = reader.read(&mut block.0)?;
        if n == 0 {
            break;
        }
        sink(&block.0[..n])?;
    }
    sink(b"new")
}
fn setup() -> (Checkpoint<Media>, SourceSnapshot, Media) {
    let media = Media(Rc::new(RefCell::new(vec![255; 65536])));
    let mut j = Checkpoint::open(media.clone()).unwrap();
    j.save(&[1; 32], &[2; 32], &vec![42; 257]).unwrap();
    let (_, source) = j
        .load_streamed_with_snapshot(&[1; 32], &mut [0; 32], |r| {
            let mut b = [0; 128];
            while r.read(&mut b)? != 0 {}
            Ok(())
        })
        .unwrap()
        .unwrap();
    (j, source, media)
}
fn drive(
    j: &mut Checkpoint<Media>,
    t: &mut IncrementalRewrite,
    a: &mut Access,
) -> Result<(), Refusal> {
    loop {
        if t.operation().is_none() {
            t.prepare_chunk(j, emit)?;
        }
        let before = a.calls;
        let done = t.advance(j, a)?;
        assert_eq!(a.calls, before + 1);
        if done {
            return Ok(());
        }
    }
}
#[test]
fn exact_completion_and_one_call_per_step() {
    let (mut j, s, _) = setup();
    let id = s.identity();
    let mut t = IncrementalRewrite::prepare(&mut j, &s, &[1; 32], &[2; 32], emit).unwrap();
    let mut a = Access {
        calls: 0,
        cut: None,
    };
    drive(&mut j, &mut t, &mut a).unwrap();
    let c = t.finish(&mut j).unwrap();
    assert!(c.continues_source(id));
    assert!(c.retains_seed());
    assert!(c.encoded_matches(
        260,
        &Sha256::digest([vec![42; 257], b"new".to_vec()].concat()).into()
    ));
    assert_eq!(j.generation(), Some(2));
}
#[test]
fn changed_staging_cannot_commit_a_self_consistent_different_payload() {
    // Exercise both an early block and the final short block. Readback alone
    // would accept either if its header digest were derived from changed data.
    for changed_block in [0, 2] {
        let (mut j, s, media) = setup();
        let old = media.0.borrow()[..32768].to_vec();
        let mut t = IncrementalRewrite::prepare(&mut j, &s, &[1; 32], &[2; 32], emit).unwrap();
        let mut a = Access {
            calls: 0,
            cut: None,
        };
        for _ in 0..9 {
            t.advance(&mut j, &mut a).unwrap();
        }
        for block in 0..3 {
            t.prepare_chunk(&mut j, emit).unwrap();
            if block == changed_block {
                t.block.0[0] ^= 1;
            }
            let result = t.advance(&mut j, &mut a);
            if block == 2 {
                assert_eq!(result, Err(Refusal::Corrupt));
            } else {
                assert_eq!(result, Ok(false));
            }
        }
        assert!(t.finish(&mut j).is_err());
        let bytes = media.0.borrow();
        assert_eq!(&bytes[..32768], old.as_slice());
        assert_eq!(&bytes[32768 + 152..32768 + 156], &[255; 4]);
        drop(bytes);
        let mut reopened = Checkpoint::open(media).unwrap();
        let mut bytes = [0; 300];
        assert_eq!(
            reopened.load(&[1; 32], &mut [0; 32], &mut bytes).unwrap(),
            Some(257)
        );
        assert_eq!(&bytes[..257], &[42; 257]);
    }
}
#[test]
fn every_physical_cut_reopens_old_or_exact_successor() {
    let (mut j, s, _) = setup();
    let mut t = IncrementalRewrite::prepare(&mut j, &s, &[1; 32], &[2; 32], emit).unwrap();
    let mut a = Access {
        calls: 0,
        cut: None,
    };
    drive(&mut j, &mut t, &mut a).unwrap();
    let count = a.calls;
    for mode in 0..3 {
        for cut in 1..=count {
            let (mut j, s, media) = setup();
            let old = media.0.borrow()[..32768].to_vec();
            let mut t = IncrementalRewrite::prepare(&mut j, &s, &[1; 32], &[2; 32], emit).unwrap();
            let mut a = Access {
                calls: 0,
                cut: Some((cut, mode)),
            };
            assert!(drive(&mut j, &mut t, &mut a).is_err());
            assert!(t.finish(&mut j).is_err());
            assert_eq!(&media.0.borrow()[..32768], old.as_slice());
            let mut reopened = Checkpoint::open(media).unwrap();
            let mut bytes = [0; 300];
            let n = reopened
                .load(&[1; 32], &mut [0; 32], &mut bytes)
                .unwrap()
                .unwrap();
            assert!(n == 257 || n == 260);
            assert_eq!(&bytes[..257], &vec![42; 257]);
            if n == 260 {
                assert_eq!(&bytes[257..260], b"new");
            }
        }
    }
    println!("{} physical cuts", count * 3);
}
#[test]
fn changed_emitter_or_source_fails_before_next_program() {
    for mutate in [false, true] {
        let (mut j, s, media) = setup();
        let mut t = IncrementalRewrite::prepare(&mut j, &s, &[1; 32], &[2; 32], emit).unwrap();
        let mut a = Access {
            calls: 0,
            cut: None,
        };
        for _ in 0..9 {
            t.advance(&mut j, &mut a).unwrap();
        }
        let count = a.calls;
        if mutate {
            media.0.borrow_mut()[HEADER as usize] ^= 1;
            assert!(t.prepare_chunk(&mut j, emit).is_err());
        } else {
            assert!(t
                .prepare_chunk(&mut j, |r, sink| {
                    emit(r, sink)?;
                    sink(b"!")
                })
                .is_err());
        }
        assert_eq!(a.calls, count);
        assert!(t.operation().is_none());
        assert!(t.block.0.iter().all(|b| *b == 0));
    }
}
#[test]
fn stale_generation_and_incomplete_source_refuse() {
    let (mut j, s, _) = setup();
    assert!(
        IncrementalRewrite::prepare(&mut j, &s, &[1; 32], &[2; 32], |_, sink| sink(b"x")).is_err()
    );
    let mut t = IncrementalRewrite::prepare(&mut j, &s, &[1; 32], &[2; 32], emit).unwrap();
    j.save(&[1; 32], &[2; 32], b"different").unwrap();
    let mut a = Access {
        calls: 0,
        cut: None,
    };
    assert!(t.advance(&mut j, &mut a).is_err());
    assert_eq!(a.calls, 0);
}
#[test]
fn cancellation_before_commit_keeps_old_bank() {
    for steps in 0..18 {
        let (mut j, s, media) = setup();
        let mut t = IncrementalRewrite::prepare(&mut j, &s, &[1; 32], &[2; 32], emit).unwrap();
        let mut a = Access {
            calls: 0,
            cut: None,
        };
        for _ in 0..steps {
            if t.operation().is_none() {
                t.prepare_chunk(&mut j, emit).unwrap();
            }
            if t.phase == Phase::Commit {
                break;
            }
            t.advance(&mut j, &mut a).unwrap();
        }
        drop(t);
        assert_eq!(Checkpoint::open(media).unwrap().generation(), Some(1));
    }
}

#[test]
fn invalid_measurement_has_no_physical_effects_and_failure_scrubs_staging() {
    let (mut journal, source, media) = setup();
    let before = media.0.borrow().clone();
    assert!(IncrementalRewrite::prepare(&mut journal, &source, &[9; 32], &[2; 32], emit).is_err());
    assert!(IncrementalRewrite::prepare(&mut journal, &source, &[1; 32], &[0; 32], emit).is_err());
    assert!(IncrementalRewrite::prepare(
        &mut journal,
        &source,
        &[1; 32],
        &[2; 32],
        |reader, sink| {
            let mut bytes = [0; 128];
            while reader.read(&mut bytes)? != 0 {}
            sink(&vec![0; 32769])
        }
    )
    .is_err());
    assert_eq!(*media.0.borrow(), before);
    let mut job =
        IncrementalRewrite::prepare(&mut journal, &source, &[1; 32], &[2; 32], emit).unwrap();
    let mut access = Access {
        calls: 0,
        cut: Some((1, 0)),
    };
    assert!(job.advance(&mut journal, &mut access).is_err());
    assert!(job.header.0.iter().all(|b| *b == 0));
    assert!(job.block.0.iter().all(|b| *b == 0));
}

#[test]
fn maximum_bank_extent_has_constant_workspace() {
    fn maximum(
        reader: &mut PayloadReader<'_, Media>,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
    ) -> Result<(), Refusal> {
        let mut bytes = [0; 128];
        while reader.read(&mut bytes)? != 0 {}
        for _ in 0..254 {
            sink(&[7; 128])?;
        }
        sink(&[7; 96])
    }
    let (mut journal, source, _) = setup();
    let mut job =
        IncrementalRewrite::prepare(&mut journal, &source, &[1; 32], &[2; 32], maximum).unwrap();
    assert!(core::mem::size_of::<IncrementalRewrite>() < 1024);
    let mut access = Access {
        calls: 0,
        cut: None,
    };
    loop {
        if job.needs_chunk() {
            job.prepare_chunk(&mut journal, maximum).unwrap();
        }
        if job.advance(&mut journal, &mut access).unwrap() {
            break;
        }
    }
    assert!(job.is_complete());
    assert_eq!(job.step() as usize, access.calls);
    let done = job.finish(&mut journal).unwrap();
    assert!(done.encoded_matches(32608, &Sha256::digest(vec![7; 32608]).into()));
}
