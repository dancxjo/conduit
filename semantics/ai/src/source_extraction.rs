//! Deterministic bounded extraction below the portable source-extraction face.

use alloc::{string::String, vec::Vec};
use conduit_core::{
    ResourceDereferenceRequirement, ResourceReferenceAccessRefusal, ResourceReferenceBinding,
};

use crate::{Chunk, ExtractionLineage, SourceRef, SourceSpan, SourceSpanUnit};

pub const TEXT_UTF8_EXTRACTION_PROFILE: &str = "extract/text-utf8@1";
pub const STRUCTURED_ITEMS_EXTRACTION_PROFILE: &str = "extract/structured-items@1";
pub const RESOURCE_METADATA_EXTRACTION_PROFILE: &str = "extract/resource-metadata@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceExtractionProfile {
    TextUtf8 { overlap_bytes: u32 },
    StructuredItems { overlap_items: u32 },
    ResourceMetadata { overlap_items: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceExtractionLimits {
    pub maximum_source_bytes: u32,
    pub maximum_source_items: u32,
    pub maximum_chunk_bytes: u32,
    pub maximum_chunks: u32,
    pub maximum_output_bytes: u32,
    pub maximum_work_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceMetadataEntry {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePayload {
    Text(Vec<u8>),
    StructuredItems(Vec<Vec<u8>>),
    ResourceMetadata(Vec<ResourceMetadataEntry>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractedSourceValue {
    Text(Vec<u8>),
    StructuredItems(Vec<Vec<u8>>),
    ResourceMetadata(Vec<ResourceMetadataEntry>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceExtractionReceipt {
    pub chunks: Vec<Chunk<ExtractedSourceValue>>,
    pub source_bytes: u32,
    pub source_items: Option<u32>,
    pub output_bytes: u32,
    pub work_units: u32,
    pub proof_class: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceExtractionRefusal {
    ResourceAccess(ResourceReferenceAccessRefusal),
    ZeroLimit,
    EmptySource,
    SourceBoundExceeded,
    SourceItemBoundExceeded,
    SourceByteExtentMismatch,
    SourceItemExtentMismatch,
    MissingItemExtent,
    PayloadProfileMismatch,
    InvalidUtf8,
    InvalidOverlap,
    ItemExceedsChunkBound,
    ChunkCountExceeded,
    OutputBoundExceeded,
    WorkBoundExceeded,
    ArithmeticOverflow,
    InvalidLineage,
}

pub fn extract_source(
    source: &SourceRef,
    dereference: &ResourceDereferenceRequirement,
    binding: &ResourceReferenceBinding,
    profile: SourceExtractionProfile,
    limits: SourceExtractionLimits,
    payload: &SourcePayload,
) -> Result<SourceExtractionReceipt, SourceExtractionRefusal> {
    validate_limits(limits)?;
    dereference
        .admit(&source.resource, binding)
        .map_err(SourceExtractionRefusal::ResourceAccess)?;
    let (source_bytes, source_items) = payload_extent(payload)?;
    if source_bytes == 0 || source_items == Some(0) {
        return Err(SourceExtractionRefusal::EmptySource);
    }
    if u64::from(source_bytes) != source.resource.extent.bytes {
        return Err(SourceExtractionRefusal::SourceByteExtentMismatch);
    }
    if source_bytes > limits.maximum_source_bytes {
        return Err(SourceExtractionRefusal::SourceBoundExceeded);
    }
    match (source.resource.extent.items, source_items) {
        (None, None) => {}
        (Some(expected), Some(actual)) if expected == u64::from(actual) => {}
        (None, Some(_)) | (Some(_), None) | (Some(_), Some(_)) => {
            return Err(SourceExtractionRefusal::SourceItemExtentMismatch)
        }
    }
    if source_items.is_some_and(|items| items > limits.maximum_source_items) {
        return Err(SourceExtractionRefusal::SourceItemBoundExceeded);
    }

    let mut receipt = SourceExtractionReceipt {
        chunks: Vec::new(),
        source_bytes,
        source_items,
        output_bytes: 0,
        work_units: source_bytes,
        proof_class: "deterministic-source-extraction",
    };
    if receipt.work_units > limits.maximum_work_units {
        return Err(SourceExtractionRefusal::WorkBoundExceeded);
    }
    match (profile, payload) {
        (SourceExtractionProfile::TextUtf8 { overlap_bytes }, SourcePayload::Text(value)) => {
            extract_text(source, value, overlap_bytes, limits, &mut receipt)?;
        }
        (
            SourceExtractionProfile::StructuredItems { overlap_items },
            SourcePayload::StructuredItems(items),
        ) => extract_items(
            source,
            items,
            overlap_items,
            STRUCTURED_ITEMS_EXTRACTION_PROFILE,
            limits,
            &mut receipt,
            ExtractedSourceValue::StructuredItems,
        )?,
        (
            SourceExtractionProfile::ResourceMetadata { overlap_items },
            SourcePayload::ResourceMetadata(items),
        ) => extract_metadata(source, items, overlap_items, limits, &mut receipt)?,
        _ => return Err(SourceExtractionRefusal::PayloadProfileMismatch),
    }
    Ok(receipt)
}

fn validate_limits(limits: SourceExtractionLimits) -> Result<(), SourceExtractionRefusal> {
    if limits.maximum_source_bytes == 0
        || limits.maximum_source_items == 0
        || limits.maximum_chunk_bytes == 0
        || limits.maximum_chunks == 0
        || limits.maximum_output_bytes == 0
        || limits.maximum_work_units == 0
    {
        return Err(SourceExtractionRefusal::ZeroLimit);
    }
    Ok(())
}

fn payload_extent(payload: &SourcePayload) -> Result<(u32, Option<u32>), SourceExtractionRefusal> {
    match payload {
        SourcePayload::Text(bytes) => Ok((u32_len(bytes.len())?, None)),
        SourcePayload::StructuredItems(items) => Ok((
            sum_lengths(items.iter().map(Vec::len))?,
            Some(u32_len(items.len())?),
        )),
        SourcePayload::ResourceMetadata(items) => Ok((
            items.iter().try_fold(0_u32, |total, item| {
                total
                    .checked_add(item.encoded_length()?)
                    .ok_or(SourceExtractionRefusal::ArithmeticOverflow)
            })?,
            Some(u32_len(items.len())?),
        )),
    }
}

fn extract_text(
    source: &SourceRef,
    bytes: &[u8],
    overlap_bytes: u32,
    limits: SourceExtractionLimits,
    receipt: &mut SourceExtractionReceipt,
) -> Result<(), SourceExtractionRefusal> {
    let text = core::str::from_utf8(bytes).map_err(|_| SourceExtractionRefusal::InvalidUtf8)?;
    if overlap_bytes >= limits.maximum_chunk_bytes {
        return Err(SourceExtractionRefusal::InvalidOverlap);
    }
    let maximum = limits.maximum_chunk_bytes as usize;
    let overlap = overlap_bytes as usize;
    let mut start = 0;
    while start < bytes.len() {
        let mut end = bytes.len().min(start.saturating_add(maximum));
        while end > start && !text.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            return Err(SourceExtractionRefusal::ItemExceedsChunkBound);
        }
        push_chunk(
            source,
            SourceSpanUnit::Bytes,
            start,
            end,
            TEXT_UTF8_EXTRACTION_PROFILE,
            ExtractedSourceValue::Text(bytes[start..end].to_vec()),
            u32_len(end - start)?,
            limits,
            receipt,
        )?;
        if end == bytes.len() {
            break;
        }
        let desired = end.saturating_sub(overlap);
        let mut next = desired;
        while next < end && !text.is_char_boundary(next) {
            next += 1;
        }
        start = if next == start { end } else { next };
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn extract_items<T>(
    source: &SourceRef,
    items: &[T],
    overlap_items: u32,
    extraction_profile: &'static str,
    limits: SourceExtractionLimits,
    receipt: &mut SourceExtractionReceipt,
    wrap: impl Fn(Vec<T>) -> ExtractedSourceValue,
) -> Result<(), SourceExtractionRefusal>
where
    T: Clone + EncodedLength,
{
    let overlap = overlap_items as usize;
    let mut start = 0;
    while start < items.len() {
        let mut end = start;
        let mut bytes = 0_u32;
        while end < items.len() {
            let item_bytes = items[end].encoded_length()?;
            if item_bytes > limits.maximum_chunk_bytes {
                return Err(SourceExtractionRefusal::ItemExceedsChunkBound);
            }
            let proposed = bytes
                .checked_add(item_bytes)
                .ok_or(SourceExtractionRefusal::ArithmeticOverflow)?;
            if proposed > limits.maximum_chunk_bytes {
                break;
            }
            bytes = proposed;
            end += 1;
        }
        if end == start {
            return Err(SourceExtractionRefusal::ItemExceedsChunkBound);
        }
        push_chunk(
            source,
            SourceSpanUnit::Items,
            start,
            end,
            extraction_profile,
            wrap(items[start..end].to_vec()),
            bytes,
            limits,
            receipt,
        )?;
        if end == items.len() {
            break;
        }
        if overlap >= end - start {
            return Err(SourceExtractionRefusal::InvalidOverlap);
        }
        start = end - overlap;
    }
    Ok(())
}

fn extract_metadata(
    source: &SourceRef,
    items: &[ResourceMetadataEntry],
    overlap_items: u32,
    limits: SourceExtractionLimits,
    receipt: &mut SourceExtractionReceipt,
) -> Result<(), SourceExtractionRefusal> {
    extract_items(
        source,
        items,
        overlap_items,
        RESOURCE_METADATA_EXTRACTION_PROFILE,
        limits,
        receipt,
        ExtractedSourceValue::ResourceMetadata,
    )
}

#[allow(clippy::too_many_arguments)]
fn push_chunk(
    source: &SourceRef,
    unit: SourceSpanUnit,
    start: usize,
    end: usize,
    extraction_profile: &'static str,
    value: ExtractedSourceValue,
    output_bytes: u32,
    limits: SourceExtractionLimits,
    receipt: &mut SourceExtractionReceipt,
) -> Result<(), SourceExtractionRefusal> {
    if receipt.chunks.len() >= limits.maximum_chunks as usize {
        return Err(SourceExtractionRefusal::ChunkCountExceeded);
    }
    receipt.output_bytes = receipt
        .output_bytes
        .checked_add(output_bytes)
        .ok_or(SourceExtractionRefusal::ArithmeticOverflow)?;
    if receipt.output_bytes > limits.maximum_output_bytes {
        return Err(SourceExtractionRefusal::OutputBoundExceeded);
    }
    receipt.work_units = receipt
        .work_units
        .checked_add(output_bytes)
        .ok_or(SourceExtractionRefusal::ArithmeticOverflow)?;
    if receipt.work_units > limits.maximum_work_units {
        return Err(SourceExtractionRefusal::WorkBoundExceeded);
    }
    let lineage = ExtractionLineage {
        source: source.clone(),
        span: SourceSpan {
            unit,
            start: start as u64,
            end: end as u64,
        },
        extraction_profile: extraction_profile.into(),
        transform_profiles: Vec::new(),
        parent_chunk: None,
    };
    receipt
        .chunks
        .push(Chunk::new(lineage, value).map_err(|_| SourceExtractionRefusal::InvalidLineage)?);
    Ok(())
}

trait EncodedLength {
    fn encoded_length(&self) -> Result<u32, SourceExtractionRefusal>;
}

impl EncodedLength for Vec<u8> {
    fn encoded_length(&self) -> Result<u32, SourceExtractionRefusal> {
        u32_len(self.len())
    }
}

impl EncodedLength for ResourceMetadataEntry {
    fn encoded_length(&self) -> Result<u32, SourceExtractionRefusal> {
        u32_len(
            self.field
                .len()
                .checked_add(self.value.len())
                .ok_or(SourceExtractionRefusal::ArithmeticOverflow)?,
        )
    }
}

fn sum_lengths(mut lengths: impl Iterator<Item = usize>) -> Result<u32, SourceExtractionRefusal> {
    lengths.try_fold(0_u32, |total, length| {
        total
            .checked_add(u32_len(length)?)
            .ok_or(SourceExtractionRefusal::ArithmeticOverflow)
    })
}

fn u32_len(length: usize) -> Result<u32, SourceExtractionRefusal> {
    length
        .try_into()
        .map_err(|_| SourceExtractionRefusal::ArithmeticOverflow)
}
