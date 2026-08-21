//! Canonical bounded encoding for deterministic source-extraction results.

use alloc::{string::String, vec::Vec};

use crate::{
    Chunk, ExtractedSourceValue, ExtractionLineage, ResourceMetadataEntry, SourceExtractionReceipt,
    SourceRef, SourceSpan, SourceSpanUnit, MAXIMUM_EXTRACTION_OUTPUT_BYTES,
    MAXIMUM_RAG_IDENTITY_BYTES,
};

const VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceExtractionCodecRefusal {
    InvalidSource,
    EmptyChunks,
    TooManyChunks,
    MixedSources,
    UnsupportedLineage,
    InvalidChunk,
    IdentityTooLarge,
    ItemCountExceeded,
    OutputTooLarge,
    AccountingMismatch,
    ArithmeticOverflow,
    Malformed,
    UnsupportedVersion,
}

impl SourceExtractionReceipt {
    pub fn encode(&self) -> Result<Vec<u8>, SourceExtractionCodecRefusal> {
        if self.chunks.is_empty() {
            return Err(SourceExtractionCodecRefusal::EmptyChunks);
        }
        if self.chunks.len() > crate::MAXIMUM_EXTRACTION_CHUNKS as usize {
            return Err(SourceExtractionCodecRefusal::TooManyChunks);
        }
        let source = &self.chunks[0].lineage.source;
        source
            .validate()
            .map_err(|_| SourceExtractionCodecRefusal::InvalidSource)?;
        validate_accounting(self, source)?;
        let source_bytes = source
            .resource
            .encode()
            .map_err(|_| SourceExtractionCodecRefusal::InvalidSource)?;
        let mut encoded = Vec::new();
        encoded.push(VERSION);
        push_u16_len(&mut encoded, source_bytes.len())?;
        encoded.extend_from_slice(&source_bytes);
        encoded.extend_from_slice(&self.source_bytes.to_le_bytes());
        push_optional_u32(&mut encoded, self.source_items);
        encoded.extend_from_slice(&self.output_bytes.to_le_bytes());
        encoded.extend_from_slice(&self.work_units.to_le_bytes());
        encoded.extend_from_slice(&(self.chunks.len() as u32).to_le_bytes());
        for chunk in &self.chunks {
            encode_chunk(&mut encoded, source, chunk)?;
            if encoded.len() > MAXIMUM_EXTRACTION_OUTPUT_BYTES as usize {
                return Err(SourceExtractionCodecRefusal::OutputTooLarge);
            }
        }
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SourceExtractionCodecRefusal> {
        if encoded.len() > MAXIMUM_EXTRACTION_OUTPUT_BYTES as usize {
            return Err(SourceExtractionCodecRefusal::OutputTooLarge);
        }
        let mut cursor = Cursor::new(encoded);
        if cursor.u8()? != VERSION {
            return Err(SourceExtractionCodecRefusal::UnsupportedVersion);
        }
        let source = SourceRef {
            resource: conduit_core::BoundedResourceRef::decode(cursor.bytes_u16()?)
                .map_err(|_| SourceExtractionCodecRefusal::InvalidSource)?,
        };
        let source_bytes = cursor.u32()?;
        let source_items = cursor.optional_u32()?;
        let output_bytes = cursor.u32()?;
        let work_units = cursor.u32()?;
        let chunk_count = cursor.u32()? as usize;
        if chunk_count == 0 {
            return Err(SourceExtractionCodecRefusal::EmptyChunks);
        }
        if chunk_count > crate::MAXIMUM_EXTRACTION_CHUNKS as usize {
            return Err(SourceExtractionCodecRefusal::TooManyChunks);
        }
        let mut chunks = Vec::with_capacity(chunk_count);
        for _ in 0..chunk_count {
            chunks.push(decode_chunk(&mut cursor, &source)?);
        }
        if !cursor.finished() {
            return Err(SourceExtractionCodecRefusal::Malformed);
        }
        let receipt = Self {
            chunks,
            source_bytes,
            source_items,
            output_bytes,
            work_units,
            proof_class: "deterministic-source-extraction",
        };
        validate_accounting(&receipt, &source)?;
        Ok(receipt)
    }
}

fn encode_chunk(
    encoded: &mut Vec<u8>,
    source: &SourceRef,
    chunk: &Chunk<ExtractedSourceValue>,
) -> Result<(), SourceExtractionCodecRefusal> {
    chunk
        .validate()
        .map_err(|_| SourceExtractionCodecRefusal::InvalidChunk)?;
    if &chunk.lineage.source != source {
        return Err(SourceExtractionCodecRefusal::MixedSources);
    }
    if !chunk.lineage.transform_profiles.is_empty() || chunk.lineage.parent_chunk.is_some() {
        return Err(SourceExtractionCodecRefusal::UnsupportedLineage);
    }
    encoded.extend_from_slice(&chunk.identity.digest());
    encoded.push(match chunk.lineage.span.unit {
        SourceSpanUnit::Bytes => 0,
        SourceSpanUnit::Items => 1,
    });
    encoded.extend_from_slice(&chunk.lineage.span.start.to_le_bytes());
    encoded.extend_from_slice(&chunk.lineage.span.end.to_le_bytes());
    push_identity(encoded, &chunk.lineage.extraction_profile)?;
    match &chunk.value {
        ExtractedSourceValue::Text(bytes) => {
            encoded.push(0);
            push_bytes_u32(encoded, bytes)?;
        }
        ExtractedSourceValue::StructuredItems(items) => {
            encoded.push(1);
            push_count(encoded, items.len())?;
            for item in items {
                push_bytes_u32(encoded, item)?;
            }
        }
        ExtractedSourceValue::ResourceMetadata(items) => {
            encoded.push(2);
            push_count(encoded, items.len())?;
            for item in items {
                push_identity(encoded, &item.field)?;
                push_identity(encoded, &item.value)?;
            }
        }
    }
    Ok(())
}

fn decode_chunk(
    cursor: &mut Cursor<'_>,
    source: &SourceRef,
) -> Result<Chunk<ExtractedSourceValue>, SourceExtractionCodecRefusal> {
    let identity = crate::ChunkIdentity::from_digest(cursor.digest()?);
    let unit = match cursor.u8()? {
        0 => SourceSpanUnit::Bytes,
        1 => SourceSpanUnit::Items,
        _ => return Err(SourceExtractionCodecRefusal::Malformed),
    };
    let span = SourceSpan {
        unit,
        start: cursor.u64()?,
        end: cursor.u64()?,
    };
    let extraction_profile = cursor.identity()?;
    let value = match cursor.u8()? {
        0 => ExtractedSourceValue::Text(cursor.bytes_u32()?.to_vec()),
        1 => {
            let count = cursor.count()?;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                items.push(cursor.bytes_u32()?.to_vec());
            }
            ExtractedSourceValue::StructuredItems(items)
        }
        2 => {
            let count = cursor.count()?;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                items.push(ResourceMetadataEntry {
                    field: cursor.identity()?,
                    value: cursor.identity()?,
                });
            }
            ExtractedSourceValue::ResourceMetadata(items)
        }
        _ => return Err(SourceExtractionCodecRefusal::Malformed),
    };
    let chunk = Chunk {
        identity,
        lineage: ExtractionLineage {
            source: source.clone(),
            span,
            extraction_profile,
            transform_profiles: Vec::new(),
            parent_chunk: None,
        },
        value,
    };
    chunk
        .validate()
        .map_err(|_| SourceExtractionCodecRefusal::InvalidChunk)?;
    Ok(chunk)
}

fn validate_accounting(
    receipt: &SourceExtractionReceipt,
    source: &SourceRef,
) -> Result<(), SourceExtractionCodecRefusal> {
    if receipt.proof_class != "deterministic-source-extraction"
        || u64::from(receipt.source_bytes) != source.resource.extent.bytes
        || receipt.source_items.map(u64::from) != source.resource.extent.items
    {
        return Err(SourceExtractionCodecRefusal::AccountingMismatch);
    }
    let output = receipt.chunks.iter().try_fold(0_u32, |total, chunk| {
        total
            .checked_add(value_bytes(&chunk.value)?)
            .ok_or(SourceExtractionCodecRefusal::ArithmeticOverflow)
    })?;
    let work = receipt
        .source_bytes
        .checked_add(output)
        .ok_or(SourceExtractionCodecRefusal::ArithmeticOverflow)?;
    if output != receipt.output_bytes || work != receipt.work_units {
        return Err(SourceExtractionCodecRefusal::AccountingMismatch);
    }
    Ok(())
}

fn value_bytes(value: &ExtractedSourceValue) -> Result<u32, SourceExtractionCodecRefusal> {
    match value {
        ExtractedSourceValue::Text(bytes) => u32_len(bytes.len()),
        ExtractedSourceValue::StructuredItems(items) => sum_lengths(items.iter().map(Vec::len)),
        ExtractedSourceValue::ResourceMetadata(items) => {
            items.iter().try_fold(0_u32, |total, item| {
                let length = item
                    .field
                    .len()
                    .checked_add(item.value.len())
                    .ok_or(SourceExtractionCodecRefusal::ArithmeticOverflow)?;
                total
                    .checked_add(u32_len(length)?)
                    .ok_or(SourceExtractionCodecRefusal::ArithmeticOverflow)
            })
        }
    }
}

fn sum_lengths(
    mut lengths: impl Iterator<Item = usize>,
) -> Result<u32, SourceExtractionCodecRefusal> {
    lengths.try_fold(0_u32, |total, length| {
        total
            .checked_add(u32_len(length)?)
            .ok_or(SourceExtractionCodecRefusal::ArithmeticOverflow)
    })
}

fn push_optional_u32(encoded: &mut Vec<u8>, value: Option<u32>) {
    match value {
        None => encoded.push(0),
        Some(value) => {
            encoded.push(1);
            encoded.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn push_identity(encoded: &mut Vec<u8>, value: &str) -> Result<(), SourceExtractionCodecRefusal> {
    if value.is_empty() || value.len() > MAXIMUM_RAG_IDENTITY_BYTES {
        return Err(SourceExtractionCodecRefusal::IdentityTooLarge);
    }
    push_u16_len(encoded, value.len())?;
    encoded.extend_from_slice(value.as_bytes());
    Ok(())
}

fn push_bytes_u32(encoded: &mut Vec<u8>, value: &[u8]) -> Result<(), SourceExtractionCodecRefusal> {
    encoded.extend_from_slice(&u32_len(value.len())?.to_le_bytes());
    encoded.extend_from_slice(value);
    Ok(())
}

fn push_count(encoded: &mut Vec<u8>, count: usize) -> Result<(), SourceExtractionCodecRefusal> {
    encoded.extend_from_slice(&u32_len(count)?.to_le_bytes());
    Ok(())
}

fn push_u16_len(encoded: &mut Vec<u8>, length: usize) -> Result<(), SourceExtractionCodecRefusal> {
    let length =
        u16::try_from(length).map_err(|_| SourceExtractionCodecRefusal::IdentityTooLarge)?;
    encoded.extend_from_slice(&length.to_le_bytes());
    Ok(())
}

fn u32_len(length: usize) -> Result<u32, SourceExtractionCodecRefusal> {
    length
        .try_into()
        .map_err(|_| SourceExtractionCodecRefusal::ArithmeticOverflow)
}

struct Cursor<'a> {
    encoded: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(encoded: &'a [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], SourceExtractionCodecRefusal> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(SourceExtractionCodecRefusal::Malformed)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(SourceExtractionCodecRefusal::Malformed)?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, SourceExtractionCodecRefusal> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, SourceExtractionCodecRefusal> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("exact cursor width"),
        ))
    }

    fn u32(&mut self) -> Result<u32, SourceExtractionCodecRefusal> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("exact cursor width"),
        ))
    }

    fn u64(&mut self) -> Result<u64, SourceExtractionCodecRefusal> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("exact cursor width"),
        ))
    }

    fn digest(&mut self) -> Result<[u8; 32], SourceExtractionCodecRefusal> {
        Ok(self.take(32)?.try_into().expect("exact digest width"))
    }

    fn bytes_u16(&mut self) -> Result<&'a [u8], SourceExtractionCodecRefusal> {
        let length = usize::from(self.u16()?);
        self.take(length)
    }

    fn bytes_u32(&mut self) -> Result<&'a [u8], SourceExtractionCodecRefusal> {
        let length = self.u32()? as usize;
        self.take(length)
    }

    fn identity(&mut self) -> Result<String, SourceExtractionCodecRefusal> {
        let value = self.bytes_u16()?;
        if value.is_empty() || value.len() > MAXIMUM_RAG_IDENTITY_BYTES {
            return Err(SourceExtractionCodecRefusal::IdentityTooLarge);
        }
        core::str::from_utf8(value)
            .map(String::from)
            .map_err(|_| SourceExtractionCodecRefusal::Malformed)
    }

    fn optional_u32(&mut self) -> Result<Option<u32>, SourceExtractionCodecRefusal> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.u32()?)),
            _ => Err(SourceExtractionCodecRefusal::Malformed),
        }
    }

    fn count(&mut self) -> Result<usize, SourceExtractionCodecRefusal> {
        let count = self.u32()? as usize;
        if count > crate::MAXIMUM_EXTRACTION_SOURCE_ITEMS as usize {
            return Err(SourceExtractionCodecRefusal::ItemCountExceeded);
        }
        Ok(count)
    }

    fn finished(&self) -> bool {
        self.offset == self.encoded.len()
    }
}
