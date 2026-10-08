//! Exact declared sample-grid projection; abstract basis, not a clock offer.
//! All numerical operations and fidelity classification execute checked Source.
use crate::generated::*;
use crate::source_execution::{AudioSourceExecution, AudioSourceExecutionRefusal, Program};
use crate::source_programs::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum AudioRateProjectionRefusal {
    Admission(NativeBindingRefusal),
    Source(AudioSourceExecutionRefusal),
    ForeignBasis,
}
impl From<AudioSourceExecutionRefusal> for AudioRateProjectionRefusal {
    fn from(value: AudioSourceExecutionRefusal) -> Self {
        Self::Source(value)
    }
}
impl From<NativeBindingRefusal> for AudioRateProjectionRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Admission(value)
    }
}

/// Immutable original request, exact executed frames and admitted result.
pub struct AudioSampleProjectionReceipt {
    original: AudioSampleProjectionRequest,
    original_canonical: Vec<u8>,
    executions: Vec<AudioSourceExecution>,
    result: AudioSampleProjectionResult,
    admitted_canonical: Vec<u8>,
    target: AudioIntegerFrameTarget,
}
impl AudioSampleProjectionReceipt {
    pub fn original(&self) -> &AudioSampleProjectionRequest {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn executions(&self) -> &[AudioSourceExecution] {
        &self.executions
    }
    pub fn result(&self) -> &AudioSampleProjectionResult {
        &self.result
    }
    pub fn admitted_canonical(&self) -> &[u8] {
        &self.admitted_canonical
    }
    pub fn integer_target(&self) -> &AudioIntegerFrameTarget {
        &self.target
    }
}
pub struct AudioCumulativeProjectionReceipt {
    original: AudioCumulativeFrameRequest,
    original_canonical: Vec<u8>,
    executions: Vec<AudioSourceExecution>,
    result: AudioCumulativeFrameResult,
    admitted_canonical: Vec<u8>,
    target: AudioIntegerFrameTarget,
}
impl AudioCumulativeProjectionReceipt {
    pub fn original(&self) -> &AudioCumulativeFrameRequest {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn executions(&self) -> &[AudioSourceExecution] {
        &self.executions
    }
    pub fn result(&self) -> &AudioCumulativeFrameResult {
        &self.result
    }
    pub fn admitted_canonical(&self) -> &[u8] {
        &self.admitted_canonical
    }
    pub fn integer_target(&self) -> &AudioIntegerFrameTarget {
        &self.target
    }
}
pub struct AudioSampleChainReceipt {
    original: AudioSampleProjectionChain,
    original_canonical: Vec<u8>,
    origin: Vec<AudioSourceExecution>,
    initial_cursor: AudioCumulativeFrameCursor,
    steps: Vec<AudioCumulativeProjectionReceipt>,
}
impl AudioSampleChainReceipt {
    pub fn original(&self) -> &AudioSampleProjectionChain {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn origin_execution(&self) -> &[AudioSourceExecution] {
        &self.origin
    }
    pub fn initial_cursor(&self) -> &AudioCumulativeFrameCursor {
        &self.initial_cursor
    }
    pub fn steps(&self) -> &[AudioCumulativeProjectionReceipt] {
        &self.steps
    }
}

/// Allocating semantic preparation/conformance seam, not a Flow or Play Back.
pub struct PreparedAudioSampleProjection {
    fraction: Program,
    projection: Program,
    fidelity: Program,
    basis: Program,
    append: Program,
    origin: Program,
}
impl PreparedAudioSampleProjection {
    pub fn new() -> Result<Self, AudioRateProjectionRefusal> {
        Ok(Self {
            fraction: Program::new(SAMPLE_FRACTION)?,
            projection: Program::new(SAMPLE_AT_RATE)?,
            fidelity: Program::new(FRAME_GRID_FIDELITY)?,
            basis: Program::new(CUMULATIVE_BASIS)?,
            append: Program::new(CUMULATIVE_APPEND)?,
            origin: Program::new(CUMULATIVE_ORIGIN)?,
        })
    }
    pub fn project(
        &self,
        canonical: &[u8],
    ) -> Result<AudioSampleProjectionReceipt, AudioRateProjectionRefusal> {
        let original = AudioSampleProjectionRequest::decode(canonical)?;
        let mut executions = Vec::new();
        let fraction: AudioSampleFraction = self
            .fraction
            .native(original.quantity().clone(), &mut executions)?;
        let eligible = AudioSampleProjectionEligible::new(fraction.clone(), original.clone())?;
        let raw: AudioSampleProjectionRaw = self.projection.native(eligible, &mut executions)?;
        let fidelity: AudioFrameGridFidelity = self.fidelity.native(
            AudioFrameFidelityInput::new(*raw.remainder_numerator())?,
            &mut executions,
        )?;
        let result = AudioSampleProjectionResult::new(fidelity, fraction, raw, original.clone())?;
        let target =
            AudioIntegerFrameTarget::new(original.basis().clone(), *result.raw().whole_frames())?;
        let admitted_canonical = result.clone().encode()?;
        Ok(AudioSampleProjectionReceipt {
            original,
            original_canonical: canonical.into(),
            executions,
            result,
            admitted_canonical,
            target,
        })
    }
    pub fn append(
        &self,
        canonical: &[u8],
    ) -> Result<AudioCumulativeProjectionReceipt, AudioRateProjectionRefusal> {
        let original = AudioCumulativeFrameRequest::decode(canonical)?;
        let mut executions = Vec::new();
        let fraction: AudioSampleFraction = self
            .fraction
            .native(original.append().quantity().clone(), &mut executions)?;
        let eligible = AudioCumulativeFrameEligible::new(fraction.clone(), original.clone())?;
        // Source rejects foreign denominator/rate/anchor/policy before arithmetic.
        if !self.basis.boolean(eligible.clone(), &mut executions)? {
            return Err(AudioRateProjectionRefusal::ForeignBasis);
        }
        let raw: AudioCumulativeFrameRaw = self.append.native(eligible, &mut executions)?;
        let fidelity: AudioFrameGridFidelity = self.fidelity.native(
            AudioFrameFidelityInput::new(*raw.remainder_numerator())?,
            &mut executions,
        )?;
        let cursor = AudioCumulativeFrameCursor::new(
            original.cursor().basis().clone(),
            *raw.remainder_numerator(),
            *raw.whole_frames(),
        )?;
        let result =
            AudioCumulativeFrameResult::new(cursor, fidelity, fraction, raw, original.clone())?;
        let target = AudioIntegerFrameTarget::new(
            original.append().basis().clone(),
            *result.raw().whole_frames(),
        )?;
        let admitted_canonical = result.clone().encode()?;
        Ok(AudioCumulativeProjectionReceipt {
            original,
            original_canonical: canonical.into(),
            executions,
            result,
            admitted_canonical,
            target,
        })
    }
    pub fn chain(
        &self,
        canonical: &[u8],
    ) -> Result<AudioSampleChainReceipt, AudioRateProjectionRefusal> {
        let original = AudioSampleProjectionChain::decode(canonical)?;
        let mut origin = Vec::new();
        let zero: AudioCumulativeFrameRaw =
            self.origin.native(original.basis().clone(), &mut origin)?;
        let initial_cursor = AudioCumulativeFrameCursor::new(
            original.basis().clone(),
            *zero.remainder_numerator(),
            *zero.whole_frames(),
        )?;
        let mut cursor = initial_cursor.clone();
        let mut steps = Vec::with_capacity(16);
        // All storage/work is bounded by the Source sequence before traversal.
        for quantity in original.quantities().as_slice() {
            let append = AudioSampleProjectionRequest::new(
                original.basis().basis().clone(),
                quantity.clone(),
            )?;
            let request = AudioCumulativeFrameRequest::new(append, cursor)?;
            let receipt = self.append(&request.encode()?)?;
            cursor = receipt.result().cursor().clone();
            steps.push(receipt);
        }
        Ok(AudioSampleChainReceipt {
            original,
            original_canonical: canonical.into(),
            origin,
            initial_cursor,
            steps,
        })
    }
}
