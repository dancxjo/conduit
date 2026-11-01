//! Lossless bounded export under the existing synchronous Face read lease.
//! No snapshot copy, transport, signature, membership or execution grant.
use super::json;
use conduit_face::Face;
use sha2::{Digest, Sha256};
#[path = "native_face_leaf_json.rs"]
mod leaf;

pub const MAXIMUM_BYTES: usize = 8192;
pub const CHUNK_BYTES: usize = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub bytes: u32,
    pub digest: [u8; 32],
    pub revision: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Unsupported,
    Capacity,
    Encoding,
    Changed,
    Offset,
}

/// The caller supplies the already validated, admitted producer Face. Hashing
/// serialized bytes does not establish that provenance or grant remote trust.
pub fn snapshot(face: &Face) -> Result<Snapshot, Refusal> {
    visit(face, 0, &mut []).map(|(snapshot, _)| snapshot)
}

/// Output is transactional: on ANY refusal discard the entire output span.
/// Every chunk rehashes the complete borrowed revision, with the same finite
/// document bound. A different current revision cannot mix into a prior export.
pub fn chunk(
    face: &Face,
    expected: Snapshot,
    offset: u32,
    output: &mut [u8; CHUNK_BYTES],
) -> Result<usize, Refusal> {
    if offset >= expected.bytes || !(offset as usize).is_multiple_of(CHUNK_BYTES) {
        return Err(Refusal::Offset);
    }
    let (actual, copied) = visit(face, offset as usize, output)?;
    if actual != expected {
        return Err(Refusal::Changed);
    }
    Ok(copied)
}

fn visit(face: &Face, offset: usize, output: &mut [u8]) -> Result<(Snapshot, usize), Refusal> {
    let view = leaf::checked(face).map_err(|_| Refusal::Unsupported)?;
    let mut hash = Sha256::new();
    let mut length = 0usize;
    let mut copied = 0usize;
    let mut capacity = false;
    json::stream(&view, &mut |bytes: &[u8]| {
        let Some(end) = length
            .checked_add(bytes.len())
            .filter(|n| *n <= MAXIMUM_BYTES)
        else {
            capacity = true;
            return Err(json::CodecError::Sink);
        };
        hash.update(bytes);
        let start = length.max(offset);
        let stop = end.min(offset.saturating_add(output.len()));
        if start < stop {
            output[start - offset..stop - offset]
                .copy_from_slice(&bytes[start - length..stop - length]);
            copied += stop - start;
        }
        length = end;
        Ok(())
    })
    .map_err(|_| {
        if capacity {
            Refusal::Capacity
        } else {
            Refusal::Encoding
        }
    })?;
    Ok((
        Snapshot {
            bytes: length as u32,
            digest: hash.finalize().into(),
            revision: face.revision,
        },
        copied,
    ))
}
