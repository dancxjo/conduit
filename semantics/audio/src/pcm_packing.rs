//! Bounded conversion from checked Native sample integers to the existing PCM wire format.
use crate::{
    AudioPcm16PackingInput, PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation,
    PCM_FRAME_HEADER_ENCODED_LEN,
};
use alloc::{boxed::Box, vec::Vec};
use conduit_core::{
    validate_canonical_structured_value, StructuredInfoRefusal,
    ValidatedCanonicalStructuredValue as View,
};
use conduit_plot::rust_binding::NativeBindingRefusal;

pub const PCM16_PACKING_MAXIMUM_SAMPLES: usize = 1024;
pub const PCM16_PACKING_MAXIMUM_OUTPUT_BYTES: usize =
    PCM_FRAME_HEADER_ENCODED_LEN + PCM16_PACKING_MAXIMUM_SAMPLES * 2;
pub const PCM16_PACKING_SOURCE: &str = include_str!("../pcm_packing_types.conduit");

#[derive(Debug)]
pub enum Pcm16PackingRefusal {
    Native(NativeBindingRefusal),
    Structured(StructuredInfoRefusal),
    InvalidHeader,
    SampleOutOfRange,
}
impl From<StructuredInfoRefusal> for Pcm16PackingRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

/// Owns all output storage before Play. A failed conversion exposes no partial block.
/// Integer narrowing is checked; clipping and sample generation belong to Source.
pub struct PreparedPcm16Packing {
    type_bytes: Vec<u8>,
    length: usize,
    output: Box<[u8; PCM16_PACKING_MAXIMUM_OUTPUT_BYTES]>,
}
impl PreparedPcm16Packing {
    pub fn prepare() -> Result<Self, Pcm16PackingRefusal> {
        let ty = AudioPcm16PackingInput::semantic_type().map_err(Pcm16PackingRefusal::Native)?;
        Ok(Self {
            type_bytes: ty.canonical_bytes()?,
            length: 0,
            output: Box::new([0; PCM16_PACKING_MAXIMUM_OUTPUT_BYTES]),
        })
    }

    /// Borrows canonical input and fixed output; performs no allocation or Host Call.
    /// `skip` emits nothing. Closing the surrounding Flow owns end-of-stream.
    pub fn pack(&mut self, encoded: &[u8]) -> Result<Option<&[u8]>, Pcm16PackingRefusal> {
        self.length = 0;
        let input = validate_canonical_structured_value(encoded)?;
        if input.type_bytes() != self.type_bytes {
            return Err(StructuredInfoRefusal::WrongType.into());
        }
        if input.variant_tag()? == "skip" {
            return Ok(None);
        }
        let block = input
            .variant_payload("block")?
            .ok_or(StructuredInfoRefusal::WrongType)?;
        let samples = field(block, "samples")?;
        let count = u16::try_from(samples.collection_length()?)
            .map_err(|_| Pcm16PackingRefusal::InvalidHeader)?;
        let rate = u32::try_from(u64_field(block, "sample_rate_hz")?)
            .map_err(|_| Pcm16PackingRefusal::InvalidHeader)?;
        let start = u64_field(block, "start_frame")?;
        start
            .checked_add(u64::from(count))
            .ok_or(Pcm16PackingRefusal::InvalidHeader)?;
        let discontinuity =
            field(block, "discontinuity")?.primitive_representation_bytes("value/bool")?[0] != 0;
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            rate,
            PcmChannelLayout::Mono,
            count,
            u64_field(block, "clock_id")?,
            start,
            discontinuity,
        )
        .map_err(|_| Pcm16PackingRefusal::InvalidHeader)?;
        for index in 0..count {
            let sample = samples
                .collection_index(index)?
                .ok_or(StructuredInfoRefusal::WrongType)?;
            let bytes = sample.primitive_representation_bytes("value/i128")?;
            let wide = i128::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| StructuredInfoRefusal::WrongType)?,
            );
            let narrow = i16::try_from(wide).map_err(|_| Pcm16PackingRefusal::SampleOutOfRange)?;
            let offset = PCM_FRAME_HEADER_ENCODED_LEN + usize::from(index) * 2;
            self.output[offset..offset + 2].copy_from_slice(&narrow.to_le_bytes());
        }
        self.output[..PCM_FRAME_HEADER_ENCODED_LEN].copy_from_slice(&header.encode());
        self.length = PCM_FRAME_HEADER_ENCODED_LEN + usize::from(count) * 2;
        Ok(self.output())
    }
    pub fn output(&self) -> Option<&[u8]> {
        (self.length != 0).then(|| &self.output[..self.length])
    }
}
fn field<'a>(value: View<'a>, name: &str) -> Result<View<'a>, StructuredInfoRefusal> {
    value
        .record_field(name)?
        .ok_or(StructuredInfoRefusal::WrongType)
}
fn u64_field(value: View<'_>, name: &str) -> Result<u64, StructuredInfoRefusal> {
    let bytes = field(value, name)?.primitive_representation_bytes("value/u64")?;
    Ok(u64::from_le_bytes(
        bytes
            .try_into()
            .map_err(|_| StructuredInfoRefusal::WrongType)?,
    ))
}
