use super::*;
use embedded_storage::nor_flash::{ErrorType, NorFlashError, NorFlashErrorKind};
use std::{vec, vec::Vec};
#[derive(Debug)]
struct Error;
impl NorFlashError for Error {
    fn kind(&self) -> NorFlashErrorKind {
        NorFlashErrorKind::Other
    }
}
struct Flash {
    bytes: Vec<u8>,
    reads: Vec<(u32, usize)>,
    writes: Vec<(u32, usize)>,
}
impl Flash {
    fn new() -> Self {
        Self {
            bytes: vec![255; FLASH_BYTES as usize],
            reads: Vec::new(),
            writes: Vec::new(),
        }
    }
}
impl ErrorType for Flash {
    type Error = Error;
}
impl ReadNorFlash for Flash {
    const READ_SIZE: usize = 4;
    fn capacity(&self) -> usize {
        self.bytes.len()
    }
    fn read(&mut self, at: u32, to: &mut [u8]) -> Result<(), Error> {
        assert_eq!(at % 4, 0);
        assert_eq!(to.len() % 4, 0);
        assert_eq!(to.as_ptr() as usize % 4, 0);
        assert!(to.len() <= PROGRAM_BYTES);
        self.reads.push((at, to.len()));
        to.copy_from_slice(&self.bytes[at as usize..at as usize + to.len()]);
        Ok(())
    }
}
impl NorFlash for Flash {
    const WRITE_SIZE: usize = 4;
    const ERASE_SIZE: usize = 4096;
    fn write(&mut self, at: u32, from: &[u8]) -> Result<(), Error> {
        assert_eq!(at % 256, 0);
        assert_eq!(from.len(), 256);
        assert_eq!(from.as_ptr() as usize % 4, 0);
        self.writes.push((at, from.len()));
        for (to, from) in self.bytes[at as usize..at as usize + from.len()]
            .iter_mut()
            .zip(from)
        {
            *to &= *from;
        }
        Ok(())
    }
    fn erase(&mut self, _: u32, _: u32) -> Result<(), Error> {
        panic!("adapter must never erase")
    }
}
fn prepared() -> SnapshotNor<Flash> {
    match SnapshotNor::prepare(
        Flash::new(),
        SlotGeometry::checked(DURABLE_START, 8192).unwrap(),
    ) {
        Ok(v) => v,
        Err(_) => panic!("geometry"),
    }
}
#[test]
fn arbitrary_reads_and_programs_use_bounded_aligned_ram_without_erasure() {
    let mut slot = prepared();
    let input = [0xa5; 256];
    slot.program(0, &input).unwrap();
    let mut output = [0; 520];
    slot.read(3, &mut output[1..514]).unwrap();
    assert_eq!(&output[1..254], &input[3..]);
    assert!(output[254..514].iter().all(|v| *v == 255));
    assert_eq!(slot.capacity(), 8192);
    assert_eq!(slot.program_unit(), 256);
    let flash = slot.into_flash();
    assert_eq!(flash.writes, [(DURABLE_START, 256)]);
    assert_eq!(flash.reads.len(), 3);
    assert!(flash
        .reads
        .iter()
        .all(|(at, len)| *at >= DURABLE_START && *at + *len as u32 <= DURABLE_START + 8192));
}
#[test]
fn all_invalid_extents_refuse_before_media_and_return_original_custody() {
    for (offset, bytes) in [
        (DURABLE_START - 256, 8192),
        (DURABLE_END - 256, 512),
        (DURABLE_START + 1, 8192),
        (u32::MAX, 8192),
        (DURABLE_START, 0),
    ] {
        assert_eq!(
            SlotGeometry::checked(offset, bytes),
            Err(NorRefusal::Geometry)
        );
    }
    let mut slot = prepared();
    assert_eq!(slot.program(1, &[0; 256]), Err(NorRefusal::Geometry));
    assert_eq!(slot.program(0, &[0; 4]), Err(NorRefusal::Geometry));
    assert_eq!(slot.program(8192, &[0; 256]), Err(NorRefusal::Capacity));
    assert_eq!(slot.read(8191, &mut [0; 2]), Err(NorRefusal::Capacity));
    assert_eq!(slot.read(u32::MAX, &mut [0; 4]), Err(NorRefusal::Capacity));
    let flash = slot.into_flash();
    assert!(flash.reads.is_empty() && flash.writes.is_empty());
    let mut small = Flash::new();
    small.bytes.truncate(1024);
    let (returned, error) =
        SnapshotNor::prepare(small, SlotGeometry::checked(DURABLE_START, 8192).unwrap())
            .err()
            .unwrap();
    assert_eq!(error, NorRefusal::Capacity);
    assert_eq!(returned.bytes.len(), 1024);
    assert!(returned.reads.is_empty() && returned.writes.is_empty());
}
