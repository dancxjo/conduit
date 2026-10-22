//! Borrowed record access for an assigned projection. This is representation
//! access, not admission: callers must validate the assigned Plan and its profile.
use crate::{
    read_u16, AssignedPlanRefusal, ASSIGNED_KERNEL_PLAN_SCHEMA, ASSIGNED_NATIVE_PLAN_SCHEMA,
    ASSIGNED_PLAN_COUNT_KINDS, ASSIGNED_PLAN_HEADER_BYTES, ASSIGNED_PLAN_SCHEMA, MAGIC,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssignedRecord<'a> {
    pub tag: u8,
    pub payload: &'a [u8],
}

/// Exact structural record expected by a trusted resident compiled layout.
/// Its digest binds bytes, not authority or an independently meaningful Plan ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssignedRecordRequirement {
    pub tag: u8,
    pub payload_digest: [u8; 32],
}
impl AssignedRecordRequirement {
    /// The boot-specific native tail is checked against actual possession by
    /// the native operation binder. The resident graph pins its presence/ABI.
    pub fn from_kernel_record(record: AssignedRecord<'_>, schema: u16) -> Self {
        if schema == ASSIGNED_NATIVE_PLAN_SCHEMA
            && record.tag == crate::ASSIGNED_HOST_CALL
            && record.payload.len() >= 47
        {
            Self::from_record(AssignedRecord {
                tag: record.tag,
                payload: &record.payload[..47],
            })
        } else {
            Self::from_record(record)
        }
    }
    pub fn from_record(record: AssignedRecord<'_>) -> Self {
        Self {
            tag: record.tag,
            payload_digest: crate::sha256::digest(record.payload),
        }
    }
}

pub struct AssignedPlanRecords<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl<'a> AssignedPlanRecords<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, AssignedPlanRefusal> {
        if bytes.len() < ASSIGNED_PLAN_HEADER_BYTES
            || usize::from(read_u16(bytes, 10)?) != bytes.len()
        {
            return Err(AssignedPlanRefusal::WrongLength);
        }
        if bytes.get(..8) != Some(MAGIC) {
            return Err(AssignedPlanRefusal::WrongMagic);
        }
        if !matches!(
            read_u16(bytes, 8)?,
            ASSIGNED_PLAN_SCHEMA | ASSIGNED_KERNEL_PLAN_SCHEMA | ASSIGNED_NATIVE_PLAN_SCHEMA
        ) {
            return Err(AssignedPlanRefusal::WrongSchema);
        }
        Ok(Self {
            bytes,
            cursor: ASSIGNED_PLAN_HEADER_BYTES,
        })
    }
}
impl<'a> Iterator for AssignedPlanRecords<'a> {
    type Item = Result<AssignedRecord<'a>, AssignedPlanRefusal>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor == self.bytes.len() {
            return None;
        }
        match read_record(self.bytes, self.cursor) {
            Ok((record, next)) => {
                self.cursor = next;
                Some(Ok(record))
            }
            Err(error) => {
                self.cursor = self.bytes.len();
                Some(Err(error))
            }
        }
    }
}

pub(crate) fn read_record(
    bytes: &[u8],
    cursor: usize,
) -> Result<(AssignedRecord<'_>, usize), AssignedPlanRefusal> {
    let tag = *bytes
        .get(cursor)
        .ok_or(AssignedPlanRefusal::MalformedRecord)?;
    let length = usize::from(read_u16(
        bytes,
        cursor
            .checked_add(1)
            .ok_or(AssignedPlanRefusal::MalformedRecord)?,
    )?);
    let start = cursor
        .checked_add(3)
        .ok_or(AssignedPlanRefusal::MalformedRecord)?;
    let end = start
        .checked_add(length)
        .ok_or(AssignedPlanRefusal::MalformedRecord)?;
    let payload = bytes
        .get(start..end)
        .ok_or(AssignedPlanRefusal::MalformedRecord)?;
    if tag == 0 || usize::from(tag) > ASSIGNED_PLAN_COUNT_KINDS {
        return Err(AssignedPlanRefusal::UnknownRecord(tag));
    }
    Ok((AssignedRecord { tag, payload }, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn field_extents_cannot_wrap_the_host_pointer_width() {
        assert_eq!(
            crate::read_u16(&[], usize::MAX),
            Err(AssignedPlanRefusal::MalformedRecord)
        );
        assert_eq!(
            crate::read_identity(&[], usize::MAX),
            Err(AssignedPlanRefusal::MalformedRecord)
        );
        assert!(read_record(&[1], 0).is_err());
    }
}
