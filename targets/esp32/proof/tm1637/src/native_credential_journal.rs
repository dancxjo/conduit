//! Private two-sector credential custody. Commit-last records tolerate power loss.
//! Integrity detects torn/corrupt storage; it grants no radio or execution authority.
//! Read v1/v2, write v2 only. Older v1 firmware CANNOT read new records: before
//! deployment Root must back up the real credential region and rebuild compatible
//! recovery/maintenance firmware. This format change alone authorizes no deploy.
pub const CAPACITY: u32 = 8192;
const SECTOR: u32 = 4096;
const LEGACY_PAYLOAD: usize = 168;
const LEGACY_BODY: usize = 192;
const LEGACY_RECORD: usize = 196;
const LEGACY_MAGIC: &[u8; 8] = b"CDWiFi01";
const PAYLOAD: usize = 401;
const BODY: usize = 428;
const RECORD: usize = 432;
const MAGIC: &[u8; 8] = b"CDWiFi02";
#[derive(Clone, Copy)]
enum Format {
    Legacy,
    Current,
}
impl Format {
    fn body(self) -> usize {
        match self {
            Self::Legacy => LEGACY_BODY,
            Self::Current => BODY,
        }
    }
    fn payload(self) -> usize {
        match self {
            Self::Legacy => LEGACY_PAYLOAD,
            Self::Current => PAYLOAD,
        }
    }
}
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
        for byte in &mut self.0 {
            // SAFETY: exclusive initialized scratch; volatile writes prevent
            // dead-store elimination when secret custody retires.
            unsafe { core::ptr::write_volatile(byte, 0) }
        }
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
    /// Caller already validated the private selected-network request or profile set.
    /// Old committed
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
        let checksum = crc(&record.0[..BODY - 4]);
        record.0[BODY - 4..BODY].copy_from_slice(&checksum.to_le_bytes());
        // Invalidate an older inactive commit before erasing its sector. A
        // power cut during erase must not expose a committed torn old header.
        let mut inactive = Secret([0; RECORD]);
        self.storage.read(slot * SECTOR, &mut inactive.0)?;
        // Validate the actual old/new layout before choosing its commit offset.
        // An interrupted previous successor may have no published format.
        validate(&inactive.0)?;
        let actual = format(&inactive.0)?.map_or(BODY, Format::body);
        self.storage
            .program(slot * SECTOR + actual as u32, &[0; 4])?;
        // Only AFTER actual commit invalidation may the other marker be cleared.
        // In v2 offset192 is payload, possibly literally "WiFi". Clear it now
        // so a torn bytewise erase cannot expose a false legacy commit after
        // erasing the v2 header. Never modify payload under a valid v2 commit.
        let other = if actual == BODY { LEGACY_BODY } else { BODY };
        self.storage
            .program(slot * SECTOR + other as u32, &[0; 4])?;
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
fn format(record: &[u8; RECORD]) -> Result<Option<Format>, Refusal> {
    if &record[..8] == LEGACY_MAGIC {
        // A v1 writer never publishes the v2 marker. In particular a damaged
        // v2 discriminator must not hide its still-committed current record by
        // masquerading as an uncommitted legacy header.
        if &record[BODY..RECORD] == COMMIT {
            return Err(Refusal::Corrupt);
        }
        return Ok(Some(Format::Legacy));
    }
    if &record[..8] == MAGIC {
        return Ok(Some(Format::Current));
    }
    // Unknown/torn headers cannot publish. A surviving commit marker with an
    // unknown header is corrupt, not a valid legacy/new record to guess at.
    if &record[LEGACY_BODY..LEGACY_RECORD] == COMMIT || &record[BODY..RECORD] == COMMIT {
        return Err(Refusal::Corrupt);
    }
    Ok(None)
}
fn validate(record: &[u8; RECORD]) -> Result<Option<(u64, usize)>, Refusal> {
    let Some(format) = format(record)? else {
        return Ok(None);
    };
    let body = format.body();
    if &record[body..body + 4] != COMMIT {
        return Ok(None);
    }
    let revision = u64::from_le_bytes(record[8..16].try_into().unwrap());
    let length = usize::from(u16::from_le_bytes(record[16..18].try_into().unwrap()));
    if revision == 0
        || length == 0
        || length > format.payload()
        || record[18..20] != [0, 0]
        || record[20 + length..body - 4].iter().any(|b| *b != 255)
        || crc(&record[..body - 4])
            != u32::from_le_bytes(record[body - 4..body].try_into().unwrap())
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
            // Enforce the central format-transition fence independently of
            // Journal implementation: never overwrite v2 payload at old192
            // while its actual commit remains published.
            let base = (at / SECTOR * SECTOR) as usize;
            if at as usize == base + LEGACY_BODY {
                assert!(
                    &self.bytes[base..base + 8] != MAGIC
                        || &self.bytes[base + BODY..base + RECORD] != COMMIT
                );
            }
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
            for budget in 0..=SECTOR as usize + BODY + 12 {
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
    fn seed(storage: &mut Flash, slot: usize, format: Format, revision: u64, wire: &[u8]) {
        assert!(wire.len() <= format.payload());
        let body = format.body();
        let mut record = Secret([255; RECORD]);
        record.0[..8].copy_from_slice(match format {
            Format::Legacy => LEGACY_MAGIC,
            Format::Current => MAGIC,
        });
        record.0[8..16].copy_from_slice(&revision.to_le_bytes());
        record.0[16..18].copy_from_slice(&(wire.len() as u16).to_le_bytes());
        record.0[18..20].fill(0);
        record.0[20..20 + wire.len()].copy_from_slice(wire);
        let checksum = crc(&record.0[..body - 4]);
        record.0[body - 4..body].copy_from_slice(&checksum.to_le_bytes());
        record.0[body..body + 4].copy_from_slice(COMMIT);
        storage.bytes[slot * SECTOR as usize..slot * SECTOR as usize + body + 4]
            .copy_from_slice(&record.0[..body + 4]);
    }
    #[test]
    fn every_byte_cut_legacy_to_v2_and_v2_rotations_preserves_active_and_exact_payload() {
        let mut successor = [0x61; PAYLOAD];
        successor[172..176].copy_from_slice(COMMIT);
        for format in [Format::Legacy, Format::Current] {
            for active in 0..2 {
                let mut base = Flash::erased();
                let mut older = [0x62; PAYLOAD];
                older[172..176].copy_from_slice(COMMIT);
                let older_length = format.payload();
                seed(&mut base, 1 - active, format, 7, &older[..older_length]);
                seed(
                    &mut base,
                    active,
                    format,
                    8,
                    b"complete-active-private-record",
                );
                let active_start = active * SECTOR as usize;
                for budget in 0..=SECTOR as usize + BODY + 12 {
                    let mut journal = open(Flash {
                        bytes: base.bytes,
                        budget,
                    });
                    let result = journal.commit(&successor);
                    assert_eq!(
                        &journal.storage.bytes[active_start..active_start + SECTOR as usize],
                        &base.bytes[active_start..active_start + SECTOR as usize]
                    );
                    let storage = Flash {
                        bytes: journal.storage.bytes,
                        budget: usize::MAX,
                    };
                    let mut recovered = open(storage);
                    let mut output = Secret([0; RECORD]);
                    let (revision, length) = recovered.read(&mut output.0).unwrap();
                    assert!(revision == 8 || revision == 9);
                    if revision == 8 {
                        assert_eq!(&output.0[..length], b"complete-active-private-record");
                    } else {
                        assert_eq!(length, PAYLOAD);
                        assert_eq!(&output.0[..length], &successor);
                    }
                    if result.is_ok() {
                        assert_eq!(revision, 9);
                    }
                }
                // The final fully funded transition must actually publish v2.
                let mut journal = open(base);
                assert_eq!(journal.commit(&successor), Ok(9));
                assert_eq!(
                    &journal.storage.bytes
                        [(1 - active) * SECTOR as usize..(1 - active) * SECTOR as usize + 8],
                    MAGIC
                );
            }
        }
    }
    #[test]
    fn each_format_strict_crc_padding_lengths_unknown_headers_duplicate_and_exhaustion() {
        for format in [Format::Legacy, Format::Current] {
            let mut base = Flash::erased();
            seed(&mut base, 0, format, 1, b"private");
            let body = format.body();
            for at in [0, 8, 16, 18, 20, 27, body - 4] {
                let mut corrupt = Flash {
                    bytes: base.bytes,
                    budget: usize::MAX,
                };
                corrupt.bytes[at] ^= 1;
                assert!(
                    matches!(Journal::open(corrupt), Err((_, Refusal::Corrupt))),
                    "offset {at}"
                );
            }
            // Corruption is still refused when its CRC was recomputed: reserved
            // bytes, padding and extent constraints are independent of CRC.
            for at in [18, 27] {
                let mut corrupt = Flash {
                    bytes: base.bytes,
                    budget: usize::MAX,
                };
                corrupt.bytes[at] ^= 1;
                let checksum = crc(&corrupt.bytes[..body - 4]);
                corrupt.bytes[body - 4..body].copy_from_slice(&checksum.to_le_bytes());
                assert!(matches!(Journal::open(corrupt), Err((_, Refusal::Corrupt))));
            }
            let mut duplicate = Flash {
                bytes: base.bytes,
                budget: usize::MAX,
            };
            seed(&mut duplicate, 1, format, 1, b"second");
            assert!(matches!(
                Journal::open(duplicate),
                Err((_, Refusal::Corrupt))
            ));
            let mut exhausted = Flash::erased();
            seed(&mut exhausted, 0, format, u64::MAX, b"private");
            let mut journal = open(exhausted);
            let before = journal.storage.bytes;
            assert_eq!(journal.commit(b"new"), Err(Refusal::Exhausted));
            assert_eq!(journal.storage.bytes, before);
            // A torn unknown header with no surviving commit never publishes.
            base.bytes[..8].fill(0);
            base.bytes[body..body + 4].fill(0);
            assert_eq!(open(base).revision(), 0);
        }
        let mut wrong_version = Flash::erased();
        seed(&mut wrong_version, 0, Format::Current, 1, b"private");
        wrong_version.bytes[..8].copy_from_slice(LEGACY_MAGIC);
        assert!(matches!(
            Journal::open(wrong_version),
            Err((_, Refusal::Corrupt))
        ));
        let mut journal = open(Flash::erased());
        assert_eq!(journal.commit(&[1; PAYLOAD + 1]), Err(Refusal::Capacity));
        assert_eq!(journal.commit(&[]), Err(Refusal::Capacity));
        assert_eq!(journal.commit(&[1; PAYLOAD]), Ok(1));
        let mut too_small = [0xa5; PAYLOAD - 1];
        assert_eq!(journal.read(&mut too_small), Err(Refusal::Capacity));
        assert_eq!(too_small, [0xa5; PAYLOAD - 1]);
    }
}
