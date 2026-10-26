//! Private two-sector credential custody. Commit-last records tolerate power loss.
//! Integrity detects torn/corrupt storage; it grants no radio or execution authority.
pub const CAPACITY: u32 = 8192;
const SECTOR: u32 = 4096;
const PAYLOAD: usize = 168;
const BODY: usize = 192;
const RECORD: usize = 196;
const MAGIC: &[u8; 8] = b"CDWiFi01";
const COMMIT: &[u8; 4] = b"WiFi";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Geometry,
    Media,
    Corrupt,
    Empty,
    Capacity,
    Exhausted,
}
pub trait Storage {
    fn capacity(&self) -> u32;
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal>;
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal>;
    fn erase_sector(&mut self, at: u32) -> Result<(), Refusal>;
}
struct Secret([u8; RECORD]);
impl Drop for Secret {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
pub struct Journal<S> {
    storage: S,
    slot: Option<u32>,
    revision: u64,
}
impl<S: Storage> Journal<S> {
    pub fn open(mut storage: S) -> Result<Self, (S, Refusal)> {
        if storage.capacity() != CAPACITY {
            return Err((storage, Refusal::Geometry));
        }
        let mut selected = None;
        let mut revision = 0;
        for slot in 0..2 {
            let mut record = Secret([0; RECORD]);
            if let Err(e) = storage.read(slot * SECTOR, &mut record.0) {
                return Err((storage, e));
            }
            match validate(&record.0) {
                Ok(Some((current, _))) => {
                    if current == revision {
                        return Err((storage, Refusal::Corrupt));
                    }
                    if current > revision {
                        selected = Some(slot);
                        revision = current;
                    }
                }
                Ok(None) => (),
                Err(e) => return Err((storage, e)),
            }
        }
        Ok(Self {
            storage,
            slot: selected,
            revision,
        })
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn read(&mut self, out: &mut [u8]) -> Result<(u64, usize), Refusal> {
        let slot = self.slot.ok_or(Refusal::Empty)?;
        let mut record = Secret([0; RECORD]);
        self.storage.read(slot * SECTOR, &mut record.0)?;
        let (revision, length) = validate(&record.0)?.ok_or(Refusal::Corrupt)?;
        if revision != self.revision {
            return Err(Refusal::Corrupt);
        }
        if out.len() < length {
            return Err(Refusal::Capacity);
        }
        out[..length].copy_from_slice(&record.0[20..20 + length]);
        Ok((revision, length))
    }
    /// Caller already validated the private canonical request. Old committed
    /// custody remains authoritative until body verification and final commit.
    pub fn commit(&mut self, wire: &[u8]) -> Result<u64, Refusal> {
        if wire.is_empty() || wire.len() > PAYLOAD {
            return Err(Refusal::Capacity);
        }
        let revision = self.revision.checked_add(1).ok_or(Refusal::Exhausted)?;
        let slot = self.slot.map_or(0, |slot| 1 - slot);
        let mut record = Secret([255; RECORD]);
        record.0[..8].copy_from_slice(MAGIC);
        record.0[8..16].copy_from_slice(&revision.to_le_bytes());
        record.0[16..18].copy_from_slice(&(wire.len() as u16).to_le_bytes());
        record.0[18..20].fill(0);
        record.0[20..20 + wire.len()].copy_from_slice(wire);
        let checksum = crc(&record.0[..188]);
        record.0[188..192].copy_from_slice(&checksum.to_le_bytes());
        // Invalidate an older inactive commit before erasing its sector. A
        // power cut during erase must not expose a committed torn old header.
        self.storage.program(slot * SECTOR + BODY as u32, &[0; 4])?;
        self.storage.erase_sector(slot * SECTOR)?;
        self.storage.program(slot * SECTOR, &record.0[..BODY])?;
        let mut readback = Secret([0; RECORD]);
        self.storage.read(slot * SECTOR, &mut readback.0[..BODY])?;
        if readback.0[..BODY] != record.0[..BODY] {
            return Err(Refusal::Media);
        }
        self.storage.program(slot * SECTOR + BODY as u32, COMMIT)?;
        self.storage.read(slot * SECTOR, &mut readback.0)?;
        if validate(&readback.0)? != Some((revision, wire.len())) {
            return Err(Refusal::Media);
        }
        self.slot = Some(slot);
        self.revision = revision;
        Ok(revision)
    }
}
fn validate(record: &[u8; RECORD]) -> Result<Option<(u64, usize)>, Refusal> {
    // Incomplete commit cannot publish a partially written successor.
    if &record[BODY..] != COMMIT {
        return Ok(None);
    }
    let revision = u64::from_le_bytes(record[8..16].try_into().unwrap());
    let length = u16::from_le_bytes(record[16..18].try_into().unwrap()) as usize;
    if &record[..8] != MAGIC
        || revision == 0
        || length == 0
        || length > PAYLOAD
        || record[18..20] != [0, 0]
        || record[20 + length..188].iter().any(|b| *b != 255)
        || crc(&record[..188]) != u32::from_le_bytes(record[188..192].try_into().unwrap())
    {
        return Err(Refusal::Corrupt);
    }
    Ok(Some((revision, length)))
}
fn crc(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Flash {
        bytes: [u8; CAPACITY as usize],
        budget: usize,
    }
    impl Flash {
        fn erased() -> Self {
            Self {
                bytes: [255; CAPACITY as usize],
                budget: usize::MAX,
            }
        }
    }
    impl Storage for Flash {
        fn capacity(&self) -> u32 {
            CAPACITY
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
            out.copy_from_slice(&self.bytes[at as usize..at as usize + out.len()]);
            Ok(())
        }
        fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
            for (i, b) in bytes.iter().enumerate() {
                if self.budget == 0 {
                    return Err(Refusal::Media);
                }
                self.budget -= 1;
                self.bytes[at as usize + i] &= *b;
            }
            Ok(())
        }
        fn erase_sector(&mut self, at: u32) -> Result<(), Refusal> {
            for i in at..at + SECTOR {
                if self.budget == 0 {
                    return Err(Refusal::Media);
                }
                self.budget -= 1;
                self.bytes[i as usize] = 255;
            }
            Ok(())
        }
    }
    fn open(storage: Flash) -> Journal<Flash> {
        Journal::open(storage).unwrap_or_else(|_| panic!("journal open"))
    }
    #[test]
    fn every_interrupted_successor_recovers_one_complete_committed_revision() {
        let mut base = open(Flash::erased());
        base.commit(b"old-private-input").unwrap();
        for round in 0..2 {
            for budget in 0..=SECTOR as usize + RECORD + 4 {
                let mut storage = Flash {
                    bytes: base.storage.bytes,
                    budget,
                };
                let mut journal = open(storage);
                let _ = journal.commit(b"new-private-input");
                storage = journal.storage;
                storage.budget = usize::MAX;
                let mut recovered = open(storage);
                let mut out = [0; PAYLOAD];
                let (revision, length) = recovered.read(&mut out).unwrap();
                assert!(revision == base.revision || revision == base.revision + 1);
                if revision == base.revision {
                    let mut expected = [0; PAYLOAD];
                    let (_, n) = base.read(&mut expected).unwrap();
                    assert_eq!(&out[..length], &expected[..n]);
                } else {
                    assert_eq!(&out[..length], b"new-private-input");
                }
            }
            if round == 0 {
                base.commit(b"second-private-input").unwrap();
            }
        }
    }
    #[test]
    fn blank_corrupt_duplicate_and_undersized_reads_fail_closed() {
        let mut journal = open(Flash::erased());
        assert_eq!(journal.read(&mut [0; PAYLOAD]), Err(Refusal::Empty));
        journal.commit(b"private").unwrap();
        assert_eq!(journal.read(&mut [0; 1]), Err(Refusal::Capacity));
        let mut corrupt = Flash {
            bytes: journal.storage.bytes,
            budget: usize::MAX,
        };
        corrupt.bytes[20] ^= 1;
        assert!(matches!(Journal::open(corrupt), Err((_, Refusal::Corrupt))));
        let first = journal.storage.bytes[..RECORD].to_vec();
        journal.storage.bytes[SECTOR as usize..SECTOR as usize + RECORD].copy_from_slice(&first);
        assert!(matches!(
            Journal::open(journal.storage),
            Err((_, Refusal::Corrupt))
        ));
    }
}
