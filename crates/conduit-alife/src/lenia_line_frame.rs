//! Exact identity envelope for one physical distributed-Lenia Line payload.

pub const LENIA_LINE_FRAME_MAX_BYTES: usize = 1_792;
pub const LENIA_LINE_SESSION_ID_BYTES: usize = 16;

const PLAN_BYTES: usize = 96;
const PLAY_BYTES: usize = 96;
const LINE_BYTES: usize = 80;
const HOST_BYTES: usize = 64;
const BOOT_BYTES: usize = 96;
const PREFIX_BYTES: usize = 32;
const IDENTITY_BYTES: usize =
    PREFIX_BYTES + PLAN_BYTES + PLAY_BYTES + LINE_BYTES + HOST_BYTES * 2 + BOOT_BYTES * 2;
const MAGIC: [u8; 4] = *b"DLN1";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LeniaLineFrameRefusal {
    BufferTooSmall,
    WrongLength,
    WrongMagic,
    WrongVersion,
    InvalidUtf8,
    EmptyIdentity,
    IdentityTooLong,
    Chunk(crate::LeniaRegionChunkRefusal),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LeniaLineFrameIdentity<'a> {
    pub plan_id: &'a str,
    pub play_id: &'a str,
    pub line_id: &'a str,
    pub source_host_id: &'a str,
    pub source_boot_id: &'a str,
    pub sink_host_id: &'a str,
    pub sink_boot_id: &'a str,
    pub session_id: [u8; LENIA_LINE_SESSION_ID_BYTES],
}

#[derive(Debug, Copy, Clone)]
pub struct LeniaLineFrameView<'a> {
    pub identity: LeniaLineFrameIdentity<'a>,
    pub chunk: crate::LeniaRegionChunkView<'a>,
}

impl LeniaLineFrameIdentity<'_> {
    pub fn encode(self, chunk: &[u8], output: &mut [u8]) -> Result<usize, LeniaLineFrameRefusal> {
        crate::LeniaRegionChunkView::decode(chunk).map_err(LeniaLineFrameRefusal::Chunk)?;
        let length = IDENTITY_BYTES
            .checked_add(chunk.len())
            .ok_or(LeniaLineFrameRefusal::WrongLength)?;
        if length > LENIA_LINE_FRAME_MAX_BYTES || output.len() < length {
            return Err(LeniaLineFrameRefusal::BufferTooSmall);
        }
        output[..length].fill(0);
        output[..4].copy_from_slice(&MAGIC);
        output[4] = 1;
        output[8..16].copy_from_slice(&(chunk.len() as u64).to_le_bytes());
        output[16..32].copy_from_slice(&self.session_id);
        let mut offset = PREFIX_BYTES;
        for (slot, value) in [
            (PLAN_BYTES, self.plan_id),
            (PLAY_BYTES, self.play_id),
            (LINE_BYTES, self.line_id),
            (HOST_BYTES, self.source_host_id),
            (BOOT_BYTES, self.source_boot_id),
            (HOST_BYTES, self.sink_host_id),
            (BOOT_BYTES, self.sink_boot_id),
        ] {
            offset = write_slot(output, offset, slot, value)?;
        }
        output[IDENTITY_BYTES..length].copy_from_slice(chunk);
        Ok(length)
    }
}

impl<'a> LeniaLineFrameView<'a> {
    pub fn decode(encoded: &'a [u8]) -> Result<Self, LeniaLineFrameRefusal> {
        if encoded.len() < IDENTITY_BYTES || encoded.len() > LENIA_LINE_FRAME_MAX_BYTES {
            return Err(LeniaLineFrameRefusal::WrongLength);
        }
        if encoded[..4] != MAGIC {
            return Err(LeniaLineFrameRefusal::WrongMagic);
        }
        if encoded[4] != 1 || encoded[5..8] != [0; 3] {
            return Err(LeniaLineFrameRefusal::WrongVersion);
        }
        let chunk_length = usize::try_from(u64::from_le_bytes(
            encoded[8..16]
                .try_into()
                .map_err(|_| LeniaLineFrameRefusal::WrongLength)?,
        ))
        .map_err(|_| LeniaLineFrameRefusal::WrongLength)?;
        if IDENTITY_BYTES.checked_add(chunk_length) != Some(encoded.len()) {
            return Err(LeniaLineFrameRefusal::WrongLength);
        }
        let session_id = encoded[16..32]
            .try_into()
            .map_err(|_| LeniaLineFrameRefusal::WrongLength)?;
        let mut offset = PREFIX_BYTES;
        let (plan_id, next) = read_slot(encoded, offset, PLAN_BYTES)?;
        offset = next;
        let (play_id, next) = read_slot(encoded, offset, PLAY_BYTES)?;
        offset = next;
        let (line_id, next) = read_slot(encoded, offset, LINE_BYTES)?;
        offset = next;
        let (source_host_id, next) = read_slot(encoded, offset, HOST_BYTES)?;
        offset = next;
        let (source_boot_id, next) = read_slot(encoded, offset, BOOT_BYTES)?;
        offset = next;
        let (sink_host_id, next) = read_slot(encoded, offset, HOST_BYTES)?;
        offset = next;
        let (sink_boot_id, _) = read_slot(encoded, offset, BOOT_BYTES)?;
        let identity = LeniaLineFrameIdentity {
            plan_id,
            play_id,
            line_id,
            source_host_id,
            source_boot_id,
            sink_host_id,
            sink_boot_id,
            session_id,
        };
        let chunk = crate::LeniaRegionChunkView::decode(&encoded[IDENTITY_BYTES..])
            .map_err(LeniaLineFrameRefusal::Chunk)?;
        Ok(Self { identity, chunk })
    }
}

fn write_slot(
    output: &mut [u8],
    offset: usize,
    slot_bytes: usize,
    value: &str,
) -> Result<usize, LeniaLineFrameRefusal> {
    if value.is_empty() {
        return Err(LeniaLineFrameRefusal::EmptyIdentity);
    }
    if value.len() + 2 > slot_bytes {
        return Err(LeniaLineFrameRefusal::IdentityTooLong);
    }
    output[offset..offset + 2].copy_from_slice(&(value.len() as u16).to_le_bytes());
    output[offset + 2..offset + 2 + value.len()].copy_from_slice(value.as_bytes());
    Ok(offset + slot_bytes)
}

fn read_slot(
    input: &[u8],
    offset: usize,
    slot_bytes: usize,
) -> Result<(&str, usize), LeniaLineFrameRefusal> {
    let end = offset
        .checked_add(slot_bytes)
        .ok_or(LeniaLineFrameRefusal::WrongLength)?;
    let slot = input
        .get(offset..end)
        .ok_or(LeniaLineFrameRefusal::WrongLength)?;
    let length = usize::from(u16::from_le_bytes(
        slot[..2]
            .try_into()
            .map_err(|_| LeniaLineFrameRefusal::WrongLength)?,
    ));
    if length == 0 || length + 2 > slot_bytes {
        return Err(LeniaLineFrameRefusal::EmptyIdentity);
    }
    let value = core::str::from_utf8(&slot[2..2 + length])
        .map_err(|_| LeniaLineFrameRefusal::InvalidUtf8)?;
    Ok((value, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        LeniaFieldId, LeniaRegion, LeniaRegionChunkHeader, LeniaRegionChunkKind, LeniaRegionId,
    };

    #[test]
    fn exact_line_envelope_round_trips_identity_and_chunk() {
        let header = LeniaRegionChunkHeader {
            kind: LeniaRegionChunkKind::Work,
            field_id: LeniaFieldId([3; 16]),
            generation: 0,
            field_width: 32,
            field_height: 32,
            region: LeniaRegion {
                id: LeniaRegionId(0),
                x: 0,
                width: 10,
            },
            halo: 13,
            total_cells: 36 * 58,
            cell_offset: 0,
            cell_count: 1,
        };
        let mut chunk = [0; crate::LENIA_REGION_CHUNK_MAX_BYTES];
        let chunk_length = header.encode(&[7], &mut chunk).unwrap();
        let identity = LeniaLineFrameIdentity {
            plan_id: "plan/one",
            play_id: "play/one",
            line_id: "line/work/0",
            source_host_id: "host/std",
            source_boot_id: "boot/std",
            sink_host_id: "host/pico",
            sink_boot_id: "boot/pico",
            session_id: [9; 16],
        };
        let mut encoded = [0; LENIA_LINE_FRAME_MAX_BYTES];
        let length = identity
            .encode(&chunk[..chunk_length], &mut encoded)
            .unwrap();
        let decoded = LeniaLineFrameView::decode(&encoded[..length]).unwrap();
        assert_eq!(decoded.identity, identity);
        assert_eq!(decoded.chunk.header, header);
        assert_eq!(decoded.chunk.cell(0), Ok(7));
    }
}
