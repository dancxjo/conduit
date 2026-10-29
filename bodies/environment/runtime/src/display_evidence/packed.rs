//! Lossless storage of the exact canonical CEV1 page for the unchanged 64-event,
//! 12-remote-event profile. This is evidence storage, never effect authority.
//!
//! CEP1 header: magic4, version:u16, count:u16, invocation:u64, all little-endian.
//! Each record: node:u16, kind:u8, flags:u8 (port=1/request=2/remote=4), followed
//! by the present port:u16, request:u32 and canonical remote13 bytes in order.
//! Sequence equals the record index. There is no padding or trailing footer.
use super::{Page, Refusal, HEADER_BYTES, MAXIMUM_EVENTS, RECORD_BYTES};
pub const MAXIMUM_REMOTE_EVENTS: usize = 12;
pub const MAXIMUM_BYTES: usize = HEADER_BYTES + MAXIMUM_EVENTS * 10 + MAXIMUM_REMOTE_EVENTS * 9;
const _: () = assert!(MAXIMUM_BYTES == 764);

/// Validate the complete ordinary page before packing. Output on refusal must
/// be discarded; no partial encoding is usable evidence.
pub fn encode(raw: &[u8], output: &mut [u8]) -> Result<usize, Refusal> {
    let page = Page::parse(raw)?;
    if page
        .records()
        .filter(|record| record.remote.is_some())
        .count()
        > MAXIMUM_REMOTE_EVENTS
    {
        return Err(Refusal::Capacity);
    }
    if output.len() < MAXIMUM_BYTES {
        return Err(Refusal::Capacity);
    }
    output[..HEADER_BYTES].copy_from_slice(&raw[..HEADER_BYTES]);
    output[..4].copy_from_slice(b"CEP1");
    let mut at = HEADER_BYTES;
    for record in raw[HEADER_BYTES..].as_chunks::<RECORD_BYTES>().0 {
        let flags = record[6] | (record[9] << 1) | (record[15] << 2);
        put(output, &mut at, &record[4..6])?;
        put(output, &mut at, &[record[14], flags])?;
        if flags & 1 != 0 {
            put(output, &mut at, &record[7..9])?;
        }
        if flags & 2 != 0 {
            put(output, &mut at, &record[10..14])?;
        }
        if flags & 4 != 0 {
            put(output, &mut at, &record[16..29])?;
        }
    }
    Ok(at)
}

/// Reconstruct the full ordinary CEV1 page and run its complete canonical and
/// semantic parser. No full-width identity or event is narrowed or omitted.
/// Output on refusal must be discarded.
pub fn decode(packed: &[u8], output: &mut [u8]) -> Result<usize, Refusal> {
    if packed.len() < HEADER_BYTES
        || packed.len() > MAXIMUM_BYTES
        || &packed[..4] != b"CEP1"
        || packed[4..6] != 1u16.to_le_bytes()
    {
        return Err(Refusal::Geometry);
    }
    let count = u16::from_le_bytes([packed[6], packed[7]]) as usize;
    if count == 0 || count > MAXIMUM_EVENTS {
        return Err(Refusal::Geometry);
    }
    if output.len() < super::MAXIMUM_BYTES {
        return Err(Refusal::Capacity);
    }
    output[..super::MAXIMUM_BYTES].fill(0);
    output[..HEADER_BYTES].copy_from_slice(&packed[..HEADER_BYTES]);
    output[..4].copy_from_slice(b"CEV1");
    let mut at = HEADER_BYTES;
    let mut remote_count = 0;
    for index in 0..count {
        let record = &mut output
            [HEADER_BYTES + index * RECORD_BYTES..HEADER_BYTES + (index + 1) * RECORD_BYTES];
        record[..4].copy_from_slice(&(index as u32).to_le_bytes());
        record[4..6].copy_from_slice(take(packed, &mut at, 2)?);
        let head = take(packed, &mut at, 2)?;
        record[14] = head[0];
        let flags = head[1];
        if flags & !7 != 0 {
            return Err(Refusal::Canonical);
        }
        if flags & 4 != 0 && (flags & 1 == 0 || flags & 2 != 0) {
            return Err(Refusal::Identity);
        }
        if flags & 1 != 0 {
            record[6] = 1;
            record[7..9].copy_from_slice(take(packed, &mut at, 2)?);
        }
        if flags & 2 != 0 {
            record[9] = 1;
            record[10..14].copy_from_slice(take(packed, &mut at, 4)?);
        }
        if flags & 4 != 0 {
            remote_count += 1;
            if remote_count > MAXIMUM_REMOTE_EVENTS {
                return Err(Refusal::Capacity);
            }
            record[15] = 1;
            record[16..29].copy_from_slice(take(packed, &mut at, 13)?);
        }
    }
    if at != packed.len() {
        return Err(Refusal::Canonical);
    }
    let length = HEADER_BYTES + count * RECORD_BYTES;
    Page::parse(&output[..length])?;
    Ok(length)
}
fn put(output: &mut [u8], at: &mut usize, bytes: &[u8]) -> Result<(), Refusal> {
    let end = at.checked_add(bytes.len()).ok_or(Refusal::Capacity)?;
    output
        .get_mut(*at..end)
        .ok_or(Refusal::Capacity)?
        .copy_from_slice(bytes);
    *at = end;
    Ok(())
}
fn take<'a>(input: &'a [u8], at: &mut usize, count: usize) -> Result<&'a [u8], Refusal> {
    let end = at.checked_add(count).ok_or(Refusal::Geometry)?;
    let value = input.get(*at..end).ok_or(Refusal::Geometry)?;
    *at = end;
    Ok(value)
}
