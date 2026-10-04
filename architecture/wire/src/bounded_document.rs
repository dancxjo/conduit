//! Finite, ordered document chunks for an already authenticated message Line.
//! Authentication and the document digest remain the caller's responsibility.

use alloc::vec::Vec;

pub const BOUNDED_DOCUMENT_HEADER_BYTES: usize = 44;
const MAGIC: &[u8; 4] = b"CBD1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedDocumentError {
    Bound,
    Header,
    Identity,
    Offset,
    ChunkCount,
}

pub fn encode_bounded_document_chunk(
    frame: &mut [u8],
    document: &[u8],
    offset: usize,
    digest: &[u8; 32],
) -> Result<(usize, usize), BoundedDocumentError> {
    if document.is_empty()
        || document.len() > u32::MAX as usize
        || frame.len() <= BOUNDED_DOCUMENT_HEADER_BYTES
        || offset >= document.len()
    {
        return Err(BoundedDocumentError::Bound);
    }
    let end = document
        .len()
        .min(offset + frame.len() - BOUNDED_DOCUMENT_HEADER_BYTES);
    frame[..4].copy_from_slice(MAGIC);
    frame[4..8].copy_from_slice(&(document.len() as u32).to_le_bytes());
    frame[8..12].copy_from_slice(&(offset as u32).to_le_bytes());
    frame[12..BOUNDED_DOCUMENT_HEADER_BYTES].copy_from_slice(digest);
    frame[BOUNDED_DOCUMENT_HEADER_BYTES..BOUNDED_DOCUMENT_HEADER_BYTES + end - offset]
        .copy_from_slice(&document[offset..end]);
    Ok((BOUNDED_DOCUMENT_HEADER_BYTES + end - offset, end))
}

pub struct BoundedDocumentAssembly {
    bytes: Vec<u8>,
    maximum_bytes: usize,
    maximum_chunks: usize,
    chunks: usize,
    identity: Option<(usize, [u8; 32])>,
}

impl BoundedDocumentAssembly {
    pub fn new(maximum_bytes: usize, maximum_chunks: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(maximum_bytes),
            maximum_bytes,
            maximum_chunks,
            chunks: 0,
            identity: None,
        }
    }

    pub fn push(&mut self, frame: &[u8]) -> Result<bool, BoundedDocumentError> {
        if frame.len() <= BOUNDED_DOCUMENT_HEADER_BYTES || &frame[..4] != MAGIC {
            return Err(BoundedDocumentError::Header);
        }
        if self.chunks == self.maximum_chunks {
            return Err(BoundedDocumentError::ChunkCount);
        }
        let total = u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(frame[8..12].try_into().unwrap()) as usize;
        let digest: [u8; 32] = frame[12..BOUNDED_DOCUMENT_HEADER_BYTES].try_into().unwrap();
        if total == 0
            || total > self.maximum_bytes
            || frame.len() - BOUNDED_DOCUMENT_HEADER_BYTES > total
        {
            return Err(BoundedDocumentError::Bound);
        }
        if offset != self.bytes.len() || offset >= total || self.bytes.len() == total {
            return Err(BoundedDocumentError::Offset);
        }
        if let Some(identity) = self.identity {
            if identity != (total, digest) {
                return Err(BoundedDocumentError::Identity);
            }
        } else {
            self.identity = Some((total, digest));
        }
        let payload = &frame[BOUNDED_DOCUMENT_HEADER_BYTES..];
        if payload.len() > total - offset {
            return Err(BoundedDocumentError::Bound);
        }
        self.bytes.extend_from_slice(payload);
        self.chunks += 1;
        Ok(self.bytes.len() == total)
    }

    pub fn completed(&self) -> Option<(&[u8], &[u8; 32])> {
        let (total, digest) = self.identity.as_ref()?;
        (self.bytes.len() == *total).then_some((self.bytes.as_slice(), digest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_reassemble_in_order_with_exact_identity_and_bounds() {
        let document = [0x5a; 41];
        let digest = [7; 32];
        let mut frame = [0; 60];
        let mut assembly = BoundedDocumentAssembly::new(41, 3);
        let (length, next) =
            encode_bounded_document_chunk(&mut frame, &document, 0, &digest).unwrap();
        assert_eq!(next, 16);
        assert!(!assembly.push(&frame[..length]).unwrap());
        let (length, next) =
            encode_bounded_document_chunk(&mut frame, &document, next, &digest).unwrap();
        assert_eq!(next, 32);
        assert!(!assembly.push(&frame[..length]).unwrap());
        let (length, next) =
            encode_bounded_document_chunk(&mut frame, &document, next, &digest).unwrap();
        assert_eq!(next, 41);
        assert!(assembly.push(&frame[..length]).unwrap());
        assert_eq!(assembly.completed(), Some((document.as_slice(), &digest)));
        assert_eq!(
            assembly.push(&frame[..length]),
            Err(BoundedDocumentError::ChunkCount)
        );
    }

    #[test]
    fn altered_offset_and_digest_refuse() {
        let document = [1; 20];
        let mut frame = [0; 60];
        let digest = [2; 32];
        let (length, next) =
            encode_bounded_document_chunk(&mut frame, &document, 0, &digest).unwrap();
        let mut assembly = BoundedDocumentAssembly::new(20, 2);
        assert!(!assembly.push(&frame[..length]).unwrap());
        let (length, _) =
            encode_bounded_document_chunk(&mut frame, &document, next, &[3; 32]).unwrap();
        assert_eq!(
            assembly.push(&frame[..length]),
            Err(BoundedDocumentError::Identity)
        );
        frame[8] = 0;
        assert_eq!(
            assembly.push(&frame[..length]),
            Err(BoundedDocumentError::Offset)
        );
    }
}
