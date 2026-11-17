//! Lossless document images; their declared extent conveys no admission.
const MAGIC: &[u8; 4] = b"CDZ1";
pub const DEFLATED_DOCUMENT_HEADER_BYTES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeflatedDocumentError {
    Header,
    Bounds,
    Stream,
}

/// Receiver-local capacities, never inferred from the received image's header.
pub struct DeflatedDocumentLimits {
    pub maximum_expanded_bytes: usize,
    pub maximum_compressed_bytes: usize,
}

/// A borrowed zlib stream and its claimed expanded extent. Construction only
/// checks representation; neither validity, meaning nor authenticity is implied.
#[derive(Clone, Copy)]
pub struct DeflatedDocument<'a> {
    compressed: &'a [u8],
    expanded_bytes: usize,
}
impl<'a> DeflatedDocument<'a> {
    pub fn from_parts(
        compressed: &'a [u8],
        expanded_bytes: usize,
    ) -> Result<Self, DeflatedDocumentError> {
        if compressed.is_empty()
            || expanded_bytes == 0
            || u32::try_from(expanded_bytes).is_err()
            || u32::try_from(compressed.len()).is_err()
        {
            return Err(DeflatedDocumentError::Bounds);
        }
        Ok(Self {
            compressed,
            expanded_bytes,
        })
    }

    pub fn parse_image(image: &'a [u8]) -> Result<Self, DeflatedDocumentError> {
        if image.len() <= DEFLATED_DOCUMENT_HEADER_BYTES || &image[..4] != MAGIC {
            return Err(DeflatedDocumentError::Header);
        }
        let length = usize::try_from(u32::from_le_bytes(image[4..8].try_into().unwrap()))
            .map_err(|_| DeflatedDocumentError::Bounds)?;
        Self::from_parts(&image[DEFLATED_DOCUMENT_HEADER_BYTES..], length)
    }

    /// Write only the image header; callers append/lend `compressed()` intact.
    pub fn write_header(&self, out: &mut [u8]) -> Result<usize, DeflatedDocumentError> {
        if out.len() < DEFLATED_DOCUMENT_HEADER_BYTES {
            return Err(DeflatedDocumentError::Bounds);
        }
        out[..4].copy_from_slice(MAGIC);
        out[4..8].copy_from_slice(&(self.expanded_bytes as u32).to_le_bytes());
        Ok(DEFLATED_DOCUMENT_HEADER_BYTES)
    }
    pub fn compressed(&self) -> &'a [u8] {
        self.compressed
    }
    pub fn expanded_bytes(&self) -> usize {
        self.expanded_bytes
    }

    /// Opt-in decoder into caller storage. Bounds refuse before writes; an
    /// invalid stream may alter output, which must remain unpublished on error.
    /// Exact consumption/output rejects suffixes, concatenation and truncation.
    /// The decoder's own working state also requires local admission. Lending
    /// and forwarding images never requires enabling or executing this decoder.
    #[cfg(feature = "deflate")]
    pub fn decode_into<'out>(
        &self,
        output: &'out mut [u8],
        limits: DeflatedDocumentLimits,
    ) -> Result<&'out [u8], DeflatedDocumentError> {
        use miniz_oxide::inflate::{
            core::{decompress, inflate_flags, DecompressorOxide},
            TINFLStatus,
        };
        if self.expanded_bytes > limits.maximum_expanded_bytes
            || self.compressed.len() > limits.maximum_compressed_bytes
            || output.len() < self.expanded_bytes
        {
            return Err(DeflatedDocumentError::Bounds);
        }
        let (status, consumed, written) = decompress(
            &mut DecompressorOxide::new(),
            self.compressed,
            &mut output[..self.expanded_bytes],
            0,
            inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
                | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
        );
        if status != TINFLStatus::Done
            || consumed != self.compressed.len()
            || written != self.expanded_bytes
        {
            return Err(DeflatedDocumentError::Stream);
        }
        Ok(&output[..written])
    }
}
