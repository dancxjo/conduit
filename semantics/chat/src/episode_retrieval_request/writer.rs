use super::{MAXIMUM_EPISODE_RETRIEVAL_REQUEST_BYTES as MAX, R};
use alloc::vec::Vec;
pub(super) fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    if bytes.len() > MAX.saturating_sub(out.len()) {
        return Err(R::ByteBound);
    }
    out.extend_from_slice(bytes);
    Ok(())
}
pub(super) fn integer(out: &mut Vec<u8>, mut value: u64) -> Result<(), R> {
    let mut buffer = [0; 20];
    let mut start = buffer.len();
    loop {
        start -= 1;
        buffer[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    put(out, &buffer[start..])
}
pub(super) fn string(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    core::str::from_utf8(bytes).map_err(|_| R::MalformedExperience)?;
    put(out, b"\"")?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &byte in bytes {
        match byte {
            b'"' => put(out, b"\\\"")?,
            b'\\' => put(out, b"\\\\")?,
            0..=31 => put(
                out,
                &[
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX[(byte >> 4) as usize],
                    HEX[(byte & 15) as usize],
                ],
            )?,
            _ => put(out, &[byte])?,
        }
    }
    put(out, b"\"")
}
