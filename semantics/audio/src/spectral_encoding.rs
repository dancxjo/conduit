//! Canonical observation shape and Quantity cells are admitted in preparation.
//! Play updates only numeric payload slots inside that exact checked envelope.
use crate::*;
use alloc::{boxed::Box, vec, vec::Vec};
use conduit_core::{validate_canonical_structured_value, StructuredInfoRefusal};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum PcmSpectralEncodingRefusal {
    Analysis(PcmSpectralRefusal),
    Native(NativeBindingRefusal),
    Structured(StructuredInfoRefusal),
}
impl From<NativeBindingRefusal> for PcmSpectralEncodingRefusal {
    fn from(v: NativeBindingRefusal) -> Self {
        Self::Native(v)
    }
}
/// One immutable profile, caller-owned FFT scratch and one finite output slot.
pub struct PreparedPcmSpectrumObservationEncoder {
    profile: PcmSpectralProfile,
    scratch: Box<PcmSpectralWorkspace>,
    output: Box<[u8]>,
    source_start: usize,
    window_start: usize,
    window_index: usize,
    digest: [usize; 32],
    amplitudes: Box<[usize]>,
    powers: Box<[usize]>,
}
impl PreparedPcmSpectrumObservationEncoder {
    pub fn prepare(profile: PcmSpectralProfile) -> Result<Self, PcmSpectralEncodingRefusal> {
        use PcmSpectralEncodingRefusal as R;
        let input_error =
            |e| R::Analysis(PcmSpectralRefusal::Input(PcmTimeAnalysisRefusal::Frame(e)));
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            profile.sample_rate_hz(),
            PcmChannelLayout::Mono,
            profile.geometry().source_frames(),
            profile.clock_id(),
            0,
            false,
        )
        .map_err(input_error)?;
        let encoded = header
            .encode_frame(&vec![
                0;
                usize::from(profile.geometry().source_frames()) * 2
            ])
            .map_err(input_error)?;
        let mut scratch: Box<PcmSpectralWorkspace> = Box::default();
        let template = profile
            .prepare_observation(&mut scratch, &encoded, 0)
            .map_err(|e| match e {
                PcmSpectralNativeRefusal::Analysis(a) => R::Analysis(a),
                PcmSpectralNativeRefusal::Native(n) => R::Native(n),
            })?;
        let output = template.encode()?.into_boxed_slice();
        let invalid = || R::Structured(StructuredInfoRefusal::WrongType);
        let view = validate_canonical_structured_value(&output).map_err(R::Structured)?;
        let offset = |bytes: &[u8]| -> Result<usize, R> {
            let n = (bytes.as_ptr() as usize)
                .checked_sub(output.as_ptr() as usize)
                .ok_or_else(invalid)?;
            if n + bytes.len() > output.len() {
                return Err(invalid());
            }
            Ok(n)
        };
        let field_offset = |name: &str, kind: &str| -> Result<usize, R> {
            offset(
                view.record_field(name)
                    .map_err(R::Structured)?
                    .ok_or_else(invalid)?
                    .primitive_representation_bytes(kind)
                    .map_err(R::Structured)?,
            )
        };
        let source_start = field_offset("source_start_frame", "value/u64")?;
        let window_start = field_offset("window_start_frame", "value/u64")?;
        let window_index = field_offset("window_index", "value/u16")?;
        let digest_view = view
            .record_field("source_digest")
            .map_err(R::Structured)?
            .ok_or_else(invalid)?;
        let mut digest = [0; 32];
        for (i, slot) in digest.iter_mut().enumerate() {
            *slot = offset(
                digest_view
                    .collection_index(i as u16)
                    .map_err(R::Structured)?
                    .ok_or_else(invalid)?
                    .primitive_representation_bytes("value/u8")
                    .map_err(R::Structured)?,
            )?;
        }
        let slots = |name: &str| -> Result<Box<[usize]>, R> {
            let sequence = view
                .record_field(name)
                .map_err(R::Structured)?
                .ok_or_else(invalid)?;
            let mut offsets =
                Vec::with_capacity(usize::from(profile.geometry().window_frames()) / 2 + 1);
            for i in 0..=usize::from(profile.geometry().window_frames()) / 2 {
                offsets.push(offset(
                    sequence
                        .collection_index(i as u16)
                        .map_err(R::Structured)?
                        .ok_or_else(invalid)?
                        .primitive_representation_bytes(conduit_core::F64_INFO_ID)
                        .map_err(R::Structured)?,
                )?);
            }
            Ok(offsets.into_boxed_slice())
        };
        let amplitudes = slots("peak_amplitudes")?;
        let powers = slots("mean_square")?;
        Ok(Self {
            profile,
            scratch,
            output,
            source_start,
            window_start,
            window_index,
            digest,
            amplitudes,
            powers,
        })
    }
    pub fn profile(&self) -> PcmSpectralProfile {
        self.profile
    }
    pub fn encoded_len(&self) -> usize {
        self.output.len()
    }
    pub fn encode_window(
        &mut self,
        input: &[u8],
        index: u16,
    ) -> Result<&[u8], PcmSpectralEncodingRefusal> {
        use PcmSpectralEncodingRefusal as R;
        let spectrum = self
            .profile
            .observe_window(&mut self.scratch, input, index)
            .map_err(R::Analysis)?;
        let interval = spectrum.interval();
        let digest = spectrum.source_digest();
        let source_start = interval.start_frame
            - u64::from(self.profile.geometry().offset(index).map_err(R::Analysis)?);
        for (bin, (amplitude, power)) in spectrum
            .bins()
            .zip(self.amplitudes.iter().zip(self.powers.iter()))
        {
            self.output[*amplitude..*amplitude + 8]
                .copy_from_slice(&bin.peak_amplitude.to_le_bytes());
            self.output[*power..*power + 8]
                .copy_from_slice(&bin.mean_square_contribution.to_le_bytes());
        }
        self.output[self.source_start..self.source_start + 8]
            .copy_from_slice(&source_start.to_le_bytes());
        self.output[self.window_start..self.window_start + 8]
            .copy_from_slice(&interval.start_frame.to_le_bytes());
        self.output[self.window_index..self.window_index + 2].copy_from_slice(&index.to_le_bytes());
        for (slot, byte) in self.digest.iter().zip(digest) {
            self.output[*slot] = byte;
        }
        Ok(&self.output)
    }
}
