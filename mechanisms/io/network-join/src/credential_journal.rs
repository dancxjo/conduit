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
#[path = "credential_journal/tests.rs"]
mod tests;
