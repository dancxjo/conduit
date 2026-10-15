//! Fixed-shape Native specialization prepared from the owning generated binding.
//! This patches primitive payload slots, never schema bytes or collection lengths.
use crate::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, PreparedPcmTimeAnalysis};
use alloc::{boxed::Box, vec, vec::Vec};
use conduit_core::{validate_canonical_structured_value, StructuredInfoRefusal};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum PcmTimeEncodingRefusal {
    Analysis(crate::PcmTimeAnalysisRefusal),
    Native(NativeBindingRefusal),
    Structured(StructuredInfoRefusal),
    WrongFrameCount,
}

/// One output envelope, sized for the exact admitted observation window.
/// Numeric offsets are discovered from validated canonical Native views during
/// preparation; runtime never constructs Types, allocates, or changes geometry.
pub struct PreparedPcmTimeObservationEncoder {
    output: Box<[u8]>,
    frames: u16,
    scalar_offsets: [usize; 9],
    digest_offsets: [usize; 32],
    sample_offsets: Box<[usize]>,
}
impl PreparedPcmTimeObservationEncoder {
    pub fn prepare(frames: u16) -> Result<Self, PcmTimeEncodingRefusal> {
        use PcmTimeEncodingRefusal as R;
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            8000,
            PcmChannelLayout::Mono,
            frames,
            1,
            0,
            false,
        )
        .map_err(|e| R::Analysis(crate::PcmTimeAnalysisRefusal::Frame(e)))?;
        let bytes = header
            .encode_frame(&vec![0; usize::from(frames) * 2])
            .map_err(|e| R::Analysis(crate::PcmTimeAnalysisRefusal::Frame(e)))?;
        let template = PreparedPcmTimeAnalysis::prepare(&bytes, 1, 8000, 0, frames)
            .map_err(R::Analysis)?
            .native_observation()
            .map_err(R::Native)?;
        let output = template.encode().map_err(R::Native)?.into_boxed_slice();
        let view = validate_canonical_structured_value(&output).map_err(R::Structured)?;
        let invalid = || R::Structured(StructuredInfoRefusal::WrongType);
        let offset = |bytes: &[u8]| -> Result<usize, R> {
            let start = (bytes.as_ptr() as usize)
                .checked_sub(output.as_ptr() as usize)
                .ok_or_else(invalid)?;
            if start + bytes.len() > output.len() {
                return Err(invalid());
            }
            Ok(start)
        };
        let mut scalar_offsets = [0; 9];
        for (slot, (name, kind)) in scalar_offsets.iter_mut().zip([
            ("absolute_peak", conduit_core::F64_INFO_ID),
            ("clock_id", "value/u64"),
            ("mean_square", conduit_core::F64_INFO_ID),
            ("rms", conduit_core::F64_INFO_ID),
            ("sample_rate_hz", "value/u32"),
            ("samples_at_or_above_full_scale", "value/u16"),
            ("square_sum", conduit_core::F64_INFO_ID),
            ("start_frame", "value/u64"),
            ("frames", "value/u16"),
        ]) {
            let field = view
                .record_field(name)
                .map_err(R::Structured)?
                .ok_or_else(invalid)?;
            *slot = offset(
                field
                    .primitive_representation_bytes(kind)
                    .map_err(R::Structured)?,
            )?;
        }
        let digest = view
            .record_field("source_digest")
            .map_err(R::Structured)?
            .ok_or_else(invalid)?;
        let mut digest_offsets = [0; 32];
        for (i, slot) in digest_offsets.iter_mut().enumerate() {
            *slot = offset(
                digest
                    .collection_index(i as u16)
                    .map_err(R::Structured)?
                    .ok_or_else(invalid)?
                    .primitive_representation_bytes("value/u8")
                    .map_err(R::Structured)?,
            )?;
        }
        let waveform = view
            .record_field("waveform")
            .map_err(R::Structured)?
            .ok_or_else(invalid)?;
        let mut samples = Vec::with_capacity(usize::from(frames));
        for i in 0..frames {
            let page = waveform
                .collection_index(i / 1024)
                .map_err(R::Structured)?
                .ok_or_else(invalid)?;
            let value = page
                .collection_index(i % 1024)
                .map_err(R::Structured)?
                .ok_or_else(invalid)?;
            samples.push(offset(
                value
                    .primitive_representation_bytes(conduit_core::F64_INFO_ID)
                    .map_err(R::Structured)?,
            )?);
        }
        Ok(Self {
            output,
            frames,
            scalar_offsets,
            digest_offsets,
            sample_offsets: samples.into_boxed_slice(),
        })
    }
    pub fn prepared_heap_bytes(&self) -> usize {
        self.output.len() + core::mem::size_of_val(self.sample_offsets.as_ref())
    }
    pub fn maximum_output_bytes(&self) -> usize {
        self.output.len()
    }
    pub fn encoded(&self) -> &[u8] {
        &self.output
    }
    pub fn encode(
        &mut self,
        analysis: &PreparedPcmTimeAnalysis<'_>,
    ) -> Result<&[u8], PcmTimeEncodingRefusal> {
        if analysis.interval().frames != self.frames {
            return Err(PcmTimeEncodingRefusal::WrongFrameCount);
        }
        let s = analysis.statistics();
        let t = analysis.interval();
        for ((offset, width), bytes) in self
            .scalar_offsets
            .iter()
            .zip([8, 8, 8, 8, 4, 2, 8, 8, 2])
            .zip([
                s.absolute_peak.to_le_bytes(),
                t.clock_id.to_le_bytes(),
                s.mean_square.to_le_bytes(),
                s.rms.to_le_bytes(),
                (u64::from(t.sample_rate_hz)).to_le_bytes(),
                u64::from(s.samples_at_or_above_full_scale).to_le_bytes(),
                s.square_sum.to_le_bytes(),
                t.start_frame.to_le_bytes(),
                u64::from(t.frames).to_le_bytes(),
            ])
        {
            self.output[*offset..*offset + width].copy_from_slice(&bytes[..width]);
        }
        for (offset, byte) in self.digest_offsets.iter().zip(analysis.source_digest()) {
            self.output[*offset] = byte;
        }
        for (offset, sample) in self.sample_offsets.iter().zip(analysis.waveform()) {
            self.output[*offset..*offset + 8].copy_from_slice(&sample.to_le_bytes());
        }
        Ok(&self.output)
    }
}
