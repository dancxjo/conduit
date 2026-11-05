//! Borrow immutable document segments without assembling a second byte buffer.
use crate::{bounded_document::MAGIC, BoundedDocumentError, BOUNDED_DOCUMENT_HEADER_BYTES};

/// A bounded byte view, not a decoded or authenticated document. This borrows
/// both the segment descriptors and their bytes; flash-backed segments need
/// neither decoding nor copying into a document-sized allocation.
///
/// The caller owns all storage and chooses the byte/segment ceilings. Encoding
/// scans at most the retained segment count and copies at most one frame's
/// payload. Digest computation, local workspace admission, authority and the
/// underlying message Line remain the caller's responsibility.
pub struct SegmentedBoundedDocument<'a> {
    segments: &'a [&'a [u8]],
    total: usize,
}

impl<'a> SegmentedBoundedDocument<'a> {
    pub fn new(
        segments: &'a [&'a [u8]],
        maximum_bytes: usize,
        maximum_segments: usize,
    ) -> Result<Self, BoundedDocumentError> {
        if segments.len() > maximum_segments {
            return Err(BoundedDocumentError::Bound);
        }
        let total = segments
            .iter()
            .try_fold(0usize, |total, segment| total.checked_add(segment.len()));
        let total = total.ok_or(BoundedDocumentError::Bound)?;
        if total == 0 || total > maximum_bytes || u32::try_from(total).is_err() {
            return Err(BoundedDocumentError::Bound);
        }
        Ok(Self { segments, total })
    }

    pub fn total_bytes(&self) -> usize {
        self.total
    }

    /// Emit the existing CBD1 format. Refusal changes no output bytes. The
    /// supplied digest must cover the complete ordered concatenation; repeating
    /// an offset is allowed here and conveys no transport replay policy.
    pub fn encode_chunk(
        &self,
        frame: &mut [u8],
        offset: usize,
        digest: &[u8; 32],
    ) -> Result<(usize, usize), BoundedDocumentError> {
        if frame.len() <= BOUNDED_DOCUMENT_HEADER_BYTES || offset >= self.total {
            return Err(BoundedDocumentError::Bound);
        }
        let payload = (self.total - offset).min(frame.len() - BOUNDED_DOCUMENT_HEADER_BYTES);
        frame[..4].copy_from_slice(MAGIC);
        frame[4..8].copy_from_slice(&(self.total as u32).to_le_bytes());
        frame[8..12].copy_from_slice(&(offset as u32).to_le_bytes());
        frame[12..BOUNDED_DOCUMENT_HEADER_BYTES].copy_from_slice(digest);
        let mut skip = offset;
        let mut written = 0;
        for segment in self.segments {
            if skip >= segment.len() {
                skip -= segment.len();
                continue;
            }
            let length = (segment.len() - skip).min(payload - written);
            let start = BOUNDED_DOCUMENT_HEADER_BYTES + written;
            frame[start..start + length].copy_from_slice(&segment[skip..skip + length]);
            written += length;
            if written == payload {
                break;
            }
            skip = 0;
        }
        debug_assert_eq!(written, payload);
        Ok((BOUNDED_DOCUMENT_HEADER_BYTES + payload, offset + payload))
    }
}
