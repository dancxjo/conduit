//! Lossless storage encoding relative to one immutable evidence-page basis.
//! The store owns and authenticates that basis together with Body/Boot/Play
//! scope. This codec grants no custody, acknowledgement or durability. Decoded
//! bytes still pass the ordinary evidence parser and signature checks.
const RAW: u16 = 1 << 15;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Extent,
    Capacity,
    Canonical,
}
/// Exact destination extent, with raw fallback when sparse replacement grows.
pub fn encoded_len(basis: &[u8], page: &[u8]) -> Result<usize, Refusal> {
    if page.is_empty() || page.len() > super::display_evidence::MAXIMUM_BYTES {
        return Err(Refusal::Extent);
    }
    if basis.len() != page.len() {
        return Ok(2 + page.len());
    }
    let changes = basis.iter().zip(page).filter(|(a, b)| a != b).count();
    Ok(2 + page.len().min(3 * changes))
}
/// No allocation or mutation of the immutable basis. Capacity refusal performs
/// no write; successful encoding commits exactly the returned prefix.
pub fn encode(basis: &[u8], page: &[u8], output: &mut [u8]) -> Result<usize, Refusal> {
    let size = encoded_len(basis, page)?;
    if output.len() < size {
        return Err(Refusal::Capacity);
    }
    let raw = size == page.len() + 2;
    let header = page.len() as u16 | if raw { RAW } else { 0 };
    output[..2].copy_from_slice(&header.to_le_bytes());
    if raw {
        output[2..size].copy_from_slice(page);
    } else {
        let mut at = 2;
        for (offset, (&before, &after)) in basis.iter().zip(page).enumerate() {
            if before != after {
                output[at..at + 2].copy_from_slice(&(offset as u16).to_le_bytes());
                output[at + 2] = after;
                at += 3;
            }
        }
    }
    Ok(size)
}
/// Validate the whole encoding before touching output. Sparse offsets must be
/// ordered, unique and non-redundant. The outer store must independently check
/// the committed record's integrity and exact basis identity before decoding.
pub fn decode(basis: &[u8], encoded: &[u8], output: &mut [u8]) -> Result<usize, Refusal> {
    let header = u16::from_le_bytes(encoded.get(..2).ok_or(Refusal::Extent)?.try_into().unwrap());
    let length = usize::from(header & !RAW);
    if length == 0 || length > super::display_evidence::MAXIMUM_BYTES {
        return Err(Refusal::Extent);
    }
    if output.len() < length {
        return Err(Refusal::Capacity);
    }
    let raw = header & RAW != 0;
    if raw {
        if encoded.len() != length + 2 {
            return Err(Refusal::Extent);
        }
    } else {
        if basis.len() != length || !(encoded.len() - 2).is_multiple_of(3) {
            return Err(Refusal::Extent);
        }
        if encoded.len() >= length + 2 {
            return Err(Refusal::Canonical);
        }
        let mut previous = None;
        for record in encoded[2..].chunks_exact(3) {
            let offset = usize::from(u16::from_le_bytes([record[0], record[1]]));
            if offset >= length
                || previous.is_some_and(|p| offset <= p)
                || basis[offset] == record[2]
            {
                return Err(Refusal::Canonical);
            }
            previous = Some(offset);
        }
    }
    if raw {
        output[..length].copy_from_slice(&encoded[2..]);
    } else {
        output[..length].copy_from_slice(basis);
        for record in encoded[2..].chunks_exact(3) {
            let offset = usize::from(u16::from_le_bytes([record[0], record[1]]));
            output[offset] = record[2];
        }
    }
    Ok(length)
}
