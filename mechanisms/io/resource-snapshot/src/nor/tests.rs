use super::*;
use crate::tests::placement;
use alloc::{vec, vec::Vec};
#[derive(Debug)]
struct Flash {
    bytes: Vec<u8>,
    unit: u32,
    budget: Option<usize>,
    calls: usize,
    skip_program: Option<u32>,
}
impl Flash {
    fn new(unit: u32) -> Self {
        Self {
            bytes: vec![255; 1024],
            unit,
            budget: None,
            calls: 0,
            skip_program: None,
        }
    }
}
impl NorStorage for Flash {
    fn capacity(&self) -> u32 {
        self.bytes.len() as u32
    }
    fn program_unit(&self) -> u32 {
        self.unit
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), NorRefusal> {
        self.calls += 1;
        out.copy_from_slice(&self.bytes[at as usize..at as usize + out.len()]);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), NorRefusal> {
        self.calls += 1;
        assert_eq!(bytes.len(), self.unit as usize);
        assert_eq!(at % self.unit, 0);
        if self.skip_program == Some(at) {
            return Ok(());
        }
        let n = self.budget.unwrap_or(bytes.len()).min(bytes.len());
        for (old, new) in self.bytes[at as usize..].iter_mut().zip(&bytes[..n]) {
            assert_eq!(*old & *new, *new);
            *old &= *new;
        }
        if let Some(budget) = &mut self.budget {
            *budget -= n;
        }
        if n != bytes.len() {
            Err(NorRefusal::Media)
        } else {
            Ok(())
        }
    }
}
fn writer() -> (conduit_core::AuthorityBinding, PreparedSnapshotRecord) {
    let (placement, reference) = placement("boot/one", true, 2);
    (
        placement.authority[0].clone(),
        PreparedSnapshotRecord::prepare(&placement, &reference).unwrap(),
    )
}
fn reader() -> (conduit_core::AuthorityBinding, PreparedSnapshotRecord) {
    let (placement, reference) = placement("boot/two", false, 2);
    (
        placement.authority[0].clone(),
        PreparedSnapshotRecord::prepare(&placement, &reference).unwrap(),
    )
}
#[test]
fn every_partial_program_cut_refuses_publication_after_fresh_boot_reopen() {
    for unit in [1, 4, 16, 256] {
        let (authority, mut codec) = writer();
        let length = codec.publication(&authority, b"[]").unwrap().len();
        let programmed = (HEADER + length).div_ceil(unit as usize) * unit as usize + unit as usize;
        for cut in 0..=programmed {
            let mut flash = Flash::new(unit);
            flash.budget = Some(cut);
            let mut slot = PreparedNorSnapshot::prepare(flash).unwrap();
            let outcome = slot.publish(&mut codec, &authority, b"[]");
            assert_eq!(outcome.is_ok(), cut == programmed, "unit={unit}, cut={cut}");
            assert_eq!(
                slot.publish(&mut codec, &authority, b"[]"),
                Err(NorRefusal::AlreadyAttempted)
            );
            let mut flash = slot.into_storage();
            flash.budget = None;
            let original = flash.bytes.clone();
            let mut reopened = PreparedNorSnapshot::prepare(flash).unwrap();
            let (authority, codec) = reader();
            if cut == programmed {
                assert_eq!(reopened.read(&codec, &authority).unwrap(), b"[]");
            } else {
                assert_eq!(
                    reopened.read(&codec, &authority),
                    Err(NorRefusal::Unpublished)
                );
            }
            assert_eq!(reopened.into_storage().bytes, original);
        }
    }
}
#[test]
fn lost_response_reopen_validates_exact_record_and_never_overwrites_existing_generation() {
    let (authority, mut codec) = writer();
    let mut slot = PreparedNorSnapshot::prepare(Flash::new(256)).unwrap();
    let maximum = slot.maximum_publish_calls();
    slot.publish(&mut codec, &authority, b"[]").unwrap();
    let flash = slot.into_storage();
    assert!(flash.calls <= maximum);
    // The caller loses every volatile result/observer. Only exact media survives.
    let original = flash.bytes.clone();
    let mut reopened = PreparedNorSnapshot::prepare(flash).unwrap();
    assert_eq!(
        reopened.publish(&mut codec, &authority, b"[]"),
        Err(NorRefusal::Occupied)
    );
    let (read_authority, reader) = reader();
    assert_eq!(reopened.read(&reader, &read_authority).unwrap(), b"[]");
    let mut flash = reopened.into_storage();
    assert_eq!(flash.bytes, original);
    flash.bytes[HEADER + 10] ^= 1;
    let mut corrupt = PreparedNorSnapshot::prepare(flash).unwrap();
    assert_eq!(
        corrupt.read(&reader, &read_authority),
        Err(NorRefusal::Corrupt)
    );
}
#[test]
fn geometry_capacity_and_authority_refuse_before_media_effects() {
    for unit in [0, 3, 512] {
        let (flash, refusal) = PreparedNorSnapshot::prepare(Flash::new(unit))
            .err()
            .unwrap();
        assert_eq!(refusal, NorRefusal::Geometry);
        assert_eq!(flash.calls, 0);
    }
    let (authority, mut codec) = writer();
    let mut flash = Flash::new(4);
    flash.bytes.resize(64, 255);
    let mut slot = PreparedNorSnapshot::prepare(flash).unwrap();
    assert_eq!(
        slot.publish(&mut codec, &authority, b"[]"),
        Err(NorRefusal::Capacity)
    );
    assert_eq!(slot.into_storage().calls, 0);
    let mut slot = PreparedNorSnapshot::prepare(Flash::new(4)).unwrap();
    let mut wrong = authority.clone();
    wrong.boot_id = "foreign".into();
    assert!(matches!(
        slot.publish(&mut codec, &wrong, b"[]"),
        Err(NorRefusal::Record(_))
    ));
    let (read_authority, reader) = reader();
    assert!(matches!(
        slot.read(&reader, &wrong),
        Err(NorRefusal::Record(_))
    ));
    assert_eq!(slot.into_storage().calls, 0);
    let mut slot = PreparedNorSnapshot::prepare(Flash::new(4)).unwrap();
    assert_eq!(
        slot.read(&reader, &read_authority),
        Err(NorRefusal::Unpublished)
    );
}
#[test]
fn logical_slot_shares_erase_block_without_erasing_neighbor() {
    struct Slot<'a>(&'a mut [u8]);
    impl NorStorage for Slot<'_> {
        fn capacity(&self) -> u32 {
            self.0.len() as u32
        }
        fn program_unit(&self) -> u32 {
            16
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), NorRefusal> {
            out.copy_from_slice(&self.0[at as usize..at as usize + out.len()]);
            Ok(())
        }
        fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), NorRefusal> {
            for (old, new) in self.0[at as usize..].iter_mut().zip(bytes) {
                assert_eq!(*old & *new, *new);
                *old &= *new;
            }
            Ok(())
        }
    }
    let mut erase_block = vec![0x53; 4096];
    erase_block[1024..2048].fill(255);
    let (authority, mut codec) = writer();
    {
        let mut slot = PreparedNorSnapshot::prepare(Slot(&mut erase_block[1024..2048]))
            .unwrap_or_else(|_| panic!("valid exclusive slot"));
        assert_eq!(slot.slot_bytes(), 1024);
        slot.publish(&mut codec, &authority, b"[]").unwrap();
        let (authority, codec) = reader();
        assert_eq!(slot.read(&codec, &authority).unwrap(), b"[]");
    }
    assert!(erase_block[..1024].iter().all(|b| *b == 0x53));
    assert!(erase_block[2048..].iter().all(|b| *b == 0x53));
}

#[test]
fn acknowledged_wrong_program_is_detected_before_marker_and_marker_itself_is_verified() {
    for failed_at in [0, 16, 1008] {
        let (authority, mut codec) = writer();
        let mut flash = Flash::new(16);
        flash.skip_program = Some(failed_at);
        let mut slot = PreparedNorSnapshot::prepare(flash).unwrap();
        assert_eq!(
            slot.publish(&mut codec, &authority, b"[]"),
            Err(NorRefusal::Corrupt)
        );
        let flash = slot.into_storage();
        assert!(flash.bytes[1008..].iter().all(|b| *b == 255));
        let mut reopened = PreparedNorSnapshot::prepare(flash).unwrap();
        let (authority, codec) = reader();
        assert_eq!(
            reopened.read(&codec, &authority),
            Err(NorRefusal::Unpublished)
        );
    }
}
