//! Bounded transfer representation referencing independently provisioned Source
//! computation records. Decoding is framing only: Core integrity, current scope,
//! resident layout, native authority and Body admission remain mandatory.
use super::assigned::ResidentSourceIdentity;
use conduit_core as c;

pub const MAXIMUM_WIRE_BYTES: usize = 8192;
pub const MAXIMUM_RECORDS: usize = 256;
pub const MAXIMUM_RESIDENT_RECORDS: usize = 32;
const MAGIC: &[u8; 8] = b"CDAPREF1";
const PREFIX: usize = 44;
const HEADER_END: usize = PREFIX + c::ASSIGNED_PLAN_HEADER_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferRefusal {
    Frame,
    Capacity,
    Source,
    Resident,
    Reference,
}
use TransferRefusal as E;

/// Borrows wire and trusted resident storage separately. Drop the staging view
/// after fixed-table admission; it is not an admitted Plan or bearer capability.
pub struct BorrowedAssignment<'a, 'b> {
    pub header: &'a [u8],
    pub records: &'b [c::AssignedRecord<'a>],
}

/// Encode existing canonical records; only exact resident payload matches become
/// references. A failed encoding leaves partial output that must not be sent.
pub fn encode(
    header: &[u8],
    records: &[c::AssignedRecord<'_>],
    source: ResidentSourceIdentity,
    resident: &[c::AssignedRecord<'_>],
    output: &mut [u8],
) -> Result<usize, E> {
    check_resident(resident)?;
    if header.len() != c::ASSIGNED_PLAN_HEADER_BYTES {
        return Err(E::Frame);
    }
    if records.is_empty() || records.len() > MAXIMUM_RECORDS {
        return Err(E::Capacity);
    }
    let mut writer = Writer { output, at: 0 };
    writer.put(MAGIC)?;
    writer.put(&[0, 0])?;
    writer.put(&(records.len() as u16).to_le_bytes())?;
    writer.put(&source.expanded_plot_digest)?;
    writer.put(header)?;
    let mut seen = [false; MAXIMUM_RESIDENT_RECORDS];
    for record in records {
        if let Some(slot) = resident.iter().position(|expected| expected == record) {
            if core::mem::replace(&mut seen[slot], true) {
                return Err(E::Reference);
            }
            writer.put(&[1, slot as u8])?;
        } else {
            let size = u16::try_from(record.payload.len()).map_err(|_| E::Capacity)?;
            writer.put(&[0, record.tag])?;
            writer.put(&size.to_le_bytes())?;
            writer.put(record.payload)?;
        }
    }
    if seen[..resident.len()].contains(&false) {
        return Err(E::Reference);
    }
    let size = writer.at;
    writer.output[8..10].copy_from_slice(&(size as u16).to_le_bytes());
    Ok(size)
}

/// Expand references into caller-owned finite record slots, without copying any
/// payload or allocating. On refusal discard all partially populated slots.
/// The caller must next run ordinary canonical and independent layout admission.
pub fn decode<'a, 'b>(
    wire: &'a [u8],
    source: ResidentSourceIdentity,
    resident: &[c::AssignedRecord<'a>],
    slots: &'b mut [c::AssignedRecord<'a>],
) -> Result<BorrowedAssignment<'a, 'b>, E> {
    check_resident(resident)?;
    if wire.len() > MAXIMUM_WIRE_BYTES {
        return Err(E::Capacity);
    }
    if wire.len() < HEADER_END || &wire[..8] != MAGIC || number(wire, 8)? != wire.len() {
        return Err(E::Frame);
    }
    if wire[12..PREFIX] != source.expanded_plot_digest {
        return Err(E::Source);
    }
    let count = number(wire, 10)?;
    if count == 0 || count > MAXIMUM_RECORDS || count > slots.len() {
        return Err(E::Capacity);
    }
    let mut at = HEADER_END;
    let mut seen = [false; MAXIMUM_RESIDENT_RECORDS];
    for output in &mut slots[..count] {
        let tag = *wire.get(at).ok_or(E::Frame)?;
        at += 1;
        *output = match tag {
            0 => {
                let tag = *wire.get(at).ok_or(E::Frame)?;
                let size = number(wire, at + 1)?;
                at += 3;
                let payload = wire.get(at..at + size).ok_or(E::Frame)?;
                at += size;
                c::AssignedRecord { tag, payload }
            }
            1 => {
                let slot = usize::from(*wire.get(at).ok_or(E::Frame)?);
                at += 1;
                let record = *resident.get(slot).ok_or(E::Reference)?;
                if core::mem::replace(&mut seen[slot], true) {
                    return Err(E::Reference);
                }
                record
            }
            _ => return Err(E::Frame),
        };
    }
    if at != wire.len() {
        return Err(E::Frame);
    }
    if seen[..resident.len()].contains(&false) {
        return Err(E::Reference);
    }
    Ok(BorrowedAssignment {
        header: &wire[PREFIX..HEADER_END],
        records: &slots[..count],
    })
}

fn check_resident(records: &[c::AssignedRecord<'_>]) -> Result<(), E> {
    if records.is_empty() || records.len() > MAXIMUM_RESIDENT_RECORDS {
        return Err(E::Resident);
    }
    let mut bytes = 0_usize;
    for (index, record) in records.iter().enumerate() {
        bytes = bytes.checked_add(record.payload.len()).ok_or(E::Resident)?;
        if record.tag != c::ASSIGNED_CONFIGURATION
            || record.payload.len() < 19
            || record.payload[18] != 3
            || bytes > usize::from(u16::MAX)
            || records[..index].contains(record)
        {
            return Err(E::Resident);
        }
    }
    Ok(())
}
fn number(bytes: &[u8], at: usize) -> Result<usize, E> {
    let pair = bytes.get(at..at + 2).ok_or(E::Frame)?;
    Ok(usize::from(u16::from_le_bytes([pair[0], pair[1]])))
}
struct Writer<'a> {
    output: &'a mut [u8],
    at: usize,
}
impl Writer<'_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), E> {
        let end = self.at.checked_add(bytes.len()).ok_or(E::Capacity)?;
        if end > MAXIMUM_WIRE_BYTES {
            return Err(E::Capacity);
        }
        self.output
            .get_mut(self.at..end)
            .ok_or(E::Capacity)?
            .copy_from_slice(bytes);
        self.at = end;
        Ok(())
    }
}
