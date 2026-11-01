use super::*;
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
struct Media(Rc<RefCell<Vec<u8>>>, Rc<RefCell<usize>>);
impl Storage for Media {
    fn capacity(&self) -> u32 {
        65536
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        *self.1.borrow_mut() += 1;
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
    cut: Option<usize>,
}
impl Access {
    fn call(&mut self) -> Result<(), Refusal> {
        self.calls += 1;
        if self.cut == Some(self.calls) {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
}
impl SectorStorage<Media> for Access {
    fn erase_sector(&mut self, s: &mut Media, b: u8, i: u8) -> Result<(), Refusal> {
        self.call()?;
        let at = b as usize * 32768 + i as usize * 4096;
        s.0.borrow_mut()[at..at + 4096].fill(255);
        Ok(())
    }
    fn program(&mut self, s: &mut Media, at: u32, b: &[u8]) -> Result<(), Refusal> {
        self.call()?;
        s.program(at, b)
    }
    fn read(&mut self, s: &mut Media, at: u32, b: &mut [u8]) -> Result<(), Refusal> {
        self.call()?;
        s.read(at, b)
    }
}
fn setup(n: usize) -> (Checkpoint<Media>, SourceSnapshot, Media) {
    let m = Media(
        Rc::new(RefCell::new(vec![255; 65536])),
        Rc::new(RefCell::new(0)),
    );
    let mut j = Checkpoint::open(m.clone()).unwrap();
    j.save(&[1; 32], &[2; 32], &vec![42; n]).unwrap();
    let (_, s) = j
        .load_streamed_with_snapshot(&[1; 32], &mut [0; 32], |r| {
            let mut b = [0; 128];
            while r.read(&mut b)? != 0 {}
            Ok(())
        })
        .unwrap()
        .unwrap();
    (j, s, m)
}
fn drive(
    j: &mut Checkpoint<Media>,
    w: &mut IncrementalRewrite,
    s: &SourceBlocks,
    a: &mut Access,
    p: &[u8],
    x: &[u8],
    z: &[u8],
) -> Result<(), Refusal> {
    loop {
        if w.needs_chunk() {
            let before = *j.storage.1.borrow();
            w.prepare_frozen_chunk(j, s, p, x, z)?;
            let reads = *j.storage.1.borrow() - before;
            assert!(
                reads <= 5,
                "header plus at most two source blocks, no full replay"
            );
        }
        let before = a.calls;
        if w.advance(j, a)? {
            assert_eq!(a.calls, before + 1);
            return Ok(());
        }
        assert_eq!(a.calls, before + 1);
    }
}
#[test]
fn frozen_partial_crossings_and_unused_source_byte_are_exact() {
    let (mut j, s, m) = setup(513);
    let witness = SourceBlocks::authenticate(&mut j, &s, 127, 385, 128).unwrap();
    assert_eq!(witness.hash_bytes(), 128);
    let expected = [b"pref".as_slice(), &vec![42; 258], b"append", b"tail"].concat();
    let digest = Sha256::digest(&expected).into();
    let mut w = IncrementalRewrite::prepare_frozen(
        &mut j,
        &witness,
        &[1; 32],
        &[2; 32],
        expected.len(),
        digest,
    )
    .unwrap();
    // Unused block 4 is not part of the recipe. It is deliberately not claimed
    // reverified on each chunk; no byte from it can enter the successor payload.
    m.0.borrow_mut()[HEADER as usize + 512] ^= 1;
    let mut a = Access {
        calls: 0,
        cut: None,
    };
    drive(
        &mut j, &mut w, &witness, &mut a, b"pref", b"append", b"tail",
    )
    .unwrap();
    let done = w.finish(&mut j).unwrap();
    assert!(done.encoded_matches(expected.len(), &digest));
    let mut bytes = Vec::new();
    j.load_streamed(&[1; 32], &mut [0; 32], |r| {
        let mut b = [0; 128];
        loop {
            let n = r.read(&mut b)?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&b[..n]);
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(bytes, expected);
}
#[test]
fn frozen_source_tamper_header_and_generation_refuse_before_program() {
    for mode in 0..3 {
        let (mut j, s, m) = setup(513);
        let witness = SourceBlocks::authenticate(&mut j, &s, 127, 385, 128).unwrap();
        let mut w = IncrementalRewrite::prepare_frozen(
            &mut j,
            &witness,
            &[1; 32],
            &[2; 32],
            258,
            Sha256::digest(vec![42; 258]).into(),
        )
        .unwrap();
        let mut a = Access {
            calls: 0,
            cut: None,
        };
        while !w.needs_chunk() {
            w.advance(&mut j, &mut a).unwrap();
        }
        match mode {
            0 => m.0.borrow_mut()[HEADER as usize + 128] ^= 1,
            1 => m.0.borrow_mut()[56] ^= 1,
            _ => j.retained.as_mut().unwrap().generation += 1,
        }
        let calls = a.calls;
        assert!(w
            .prepare_frozen_chunk(&mut j, &witness, &[], &[], &[])
            .is_err());
        assert_eq!(a.calls, calls);
        assert!(w.block.0.iter().all(|b| *b == 0));
        assert!(w.advance(&mut j, &mut a).is_err());
        assert_eq!(a.calls, calls);
    }
}
#[test]
fn frozen_bound_capacity_and_full_source_authenticate_before_effects() {
    let (mut j, s, m) = setup(513);
    let before = *m.1.borrow();
    assert!(matches!(
        SourceBlocks::authenticate(&mut j, &s, 127, 385, 127),
        Err(Refusal::Capacity)
    ));
    assert_eq!(*m.1.borrow(), before);
    assert!(matches!(
        SourceBlocks::authenticate(&mut j, &s, 0, 514, 1024),
        Err(Refusal::Geometry)
    ));
    m.0.borrow_mut()[HEADER as usize + 512] ^= 1;
    assert!(matches!(
        SourceBlocks::authenticate(&mut j, &s, 0, 128, 32),
        Err(Refusal::Corrupt)
    ));
}
#[test]
fn frozen_maximum_index_is_explicit_and_partial_final_block_checked() {
    let n = (BANK - HEADER) as usize;
    let (mut j, s, m) = setup(n);
    let count = n.div_ceil(128);
    let witness = SourceBlocks::authenticate(&mut j, &s, 0, n, count * 32).unwrap();
    assert_eq!(witness.hash_bytes(), count * 32);
    let before = *m.1.borrow();
    let mut out = [0; 7];
    witness.read_slice(&mut j, n - 7, &mut out).unwrap();
    assert_eq!(out, [42; 7]);
    assert_eq!(*m.1.borrow() - before, 2);
    m.0.borrow_mut()[HEADER as usize + n - 1] ^= 1;
    assert!(witness.read_slice(&mut j, n - 7, &mut out).is_err());
}
#[test]
fn frozen_every_physical_cut_keeps_old_or_exact_successor() {
    let (mut j, s, _) = setup(257);
    let witness = SourceBlocks::authenticate(&mut j, &s, 0, 257, 96).unwrap();
    let expected = Sha256::digest([vec![42; 257], b"new".to_vec()].concat()).into();
    let mut w =
        IncrementalRewrite::prepare_frozen(&mut j, &witness, &[1; 32], &[2; 32], 260, expected)
            .unwrap();
    let mut a = Access {
        calls: 0,
        cut: None,
    };
    drive(&mut j, &mut w, &witness, &mut a, &[], b"new", &[]).unwrap();
    let total = a.calls;
    for cut in 1..=total {
        let (mut j, s, m) = setup(257);
        let witness = SourceBlocks::authenticate(&mut j, &s, 0, 257, 96).unwrap();
        let mut w =
            IncrementalRewrite::prepare_frozen(&mut j, &witness, &[1; 32], &[2; 32], 260, expected)
                .unwrap();
        let mut a = Access {
            calls: 0,
            cut: Some(cut),
        };
        assert!(drive(&mut j, &mut w, &witness, &mut a, &[], b"new", &[]).is_err());
        let mut reopened = Checkpoint::open(m).unwrap();
        let generation = reopened.generation().unwrap();
        assert!(matches!(generation, 1 | 2));
        let mut payload = Vec::new();
        reopened
            .load_streamed(&[1; 32], &mut [0; 32], |reader| {
                let mut block = [0; 128];
                loop {
                    let n = reader.read(&mut block)?;
                    if n == 0 {
                        break;
                    }
                    payload.extend_from_slice(&block[..n]);
                }
                Ok(())
            })
            .unwrap();
        let mut exact = vec![42; 257];
        if generation == 2 {
            exact.extend_from_slice(b"new");
        }
        assert_eq!(payload, exact);
    }
    println!("frozen_checkpoint_physical_cuts={total} old_or_exact_successor=true");
}

#[test]
fn frozen_changed_pieces_cannot_commit_an_unprepared_output() {
    let (mut j, source, media) = setup(257);
    let witness = SourceBlocks::authenticate(&mut j, &source, 0, 257, 96).unwrap();
    let expected: [u8; 32] = Sha256::digest([vec![42; 257], b"new".to_vec()].concat()).into();
    let mut writer =
        IncrementalRewrite::prepare_frozen(&mut j, &witness, &[1; 32], &[2; 32], 260, expected)
            .unwrap();
    let mut access = Access {
        calls: 0,
        cut: None,
    };
    assert!(matches!(
        drive(&mut j, &mut writer, &witness, &mut access, &[], b"bad", &[]),
        Err(Refusal::Corrupt)
    ));
    assert_eq!(Checkpoint::open(media).unwrap().generation(), Some(1));
}
