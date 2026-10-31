//! Retain one input Value until its original finite frames have drained.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_audio::{
    PcmClipError, PcmClipView, MAXIMUM_PCM_CLIP_BLOCKS, MAXIMUM_PCM_CLIP_BYTES,
    PCM_CLIP_FRAMES_MAXIMUM_OUTPUT_BYTES,
};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::PCM_CLIP_FRAMES_IMPLEMENTATION,
    budget,
    prepare,
};
#[derive(Clone, Copy, Default)]
struct FrameSpan {
    offset: u32,
    length: u32,
}
pub(super) struct PcmClipFramesBack {
    spans: Box<[FrameSpan; MAXIMUM_PCM_CLIP_BLOCKS]>,
    output: Box<[u8]>,
    output_len: usize,
    source: Option<ValueRef>,
    next: usize,
    count: usize,
    closed: bool,
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
fn invalid_clip(reason: PcmClipError) -> StepOutcome {
    fail(
        FailureCode::InvalidInput,
        match reason {
            PcmClipError::Empty => 1,
            PcmClipError::TooManyBlocks => 2,
            PcmClipError::BoundExceeded => 3,
            PcmClipError::MalformedFrame => 4,
            PcmClipError::MixedProfile => 5,
            PcmClipError::MixedClock => 6,
            PcmClipError::Discontinuity => 7,
            PcmClipError::NonContiguous => 8,
            PcmClipError::FrameCountOverflow => 9,
            PcmClipError::DeclaredExtentMismatch => 10,
            PcmClipError::TrailingBytes => 11,
        },
    )
}
impl PcmClipFramesBack {
    fn new() -> Self {
        Self {
            spans: Box::new([FrameSpan::default(); MAXIMUM_PCM_CLIP_BLOCKS]),
            output: vec![0; PCM_CLIP_FRAMES_MAXIMUM_OUTPUT_BYTES as usize].into_boxed_slice(),
            output_len: 0,
            source: None,
            next: 0,
            count: 0,
            closed: false,
        }
    }
    pub(super) fn allocation_capacity(&self) -> usize {
        self.output.len() + core::mem::size_of_val(self.spans.as_ref())
    }
}
impl<const PORTS: usize> StepBack<PORTS> for PcmClipFramesBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.closed {
            return StepOutcome::Complete;
        }
        let Some(value) = io.input(PortId(0)) else {
            return if self.source.is_some() {
                fail(FailureCode::InvalidLifecycle, 1)
            } else if io.input_closed(PortId(0)) {
                fail(FailureCode::InvalidInput, 12)
            } else {
                StepOutcome::Await
            };
        };
        if self.source.is_some_and(|source| source != value) {
            return fail(FailureCode::InvalidLifecycle, 2);
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = inputs.input(PortId(0)) else {
            return fail(FailureCode::InvalidInput, 12);
        };
        if self.source.is_none() {
            let clip = match PcmClipView::decode(bytes) {
                Ok(clip) => clip,
                Err(reason) => return invalid_clip(reason),
            };
            // Cache only ranges, never PCM bytes or pointers into transient
            // StepInputBytes. Kernel custody keeps the unconsumed Value immutable.
            for (i, block) in clip.encoded_frames().enumerate() {
                let Some(offset) = (block.as_ptr() as usize).checked_sub(bytes.as_ptr() as usize)
                else {
                    return fail(FailureCode::InvalidInput, 12);
                };
                if block.len() > self.output.len() {
                    return fail(FailureCode::InvalidInput, 13);
                }
                self.spans[i] = FrameSpan {
                    offset: offset as u32,
                    length: block.len() as u32,
                };
            }
            self.count = clip.block_count();
            self.source = Some(value);
        }
        let span = self.spans[self.next];
        let start = span.offset as usize;
        let Some(block) = bytes.get(start..start + span.length as usize) else {
            return fail(FailureCode::InvalidLifecycle, 3);
        };
        self.output[..block.len()].copy_from_slice(block);
        self.output_len = block.len();
        io.send_prepared(PortId(0), span.length)
            .expect("ready finite PCM frame output");
        self.next += 1;
        if self.next == self.count {
            io.consume(PortId(0)).expect("retained source clip");
            self.closed = true;
            self.source = None;
        }
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then(|| &self.output[..self.output_len])
    }
    fn cancel(&mut self) {
        self.closed = true;
        self.source = None;
        self.output_len = 0;
    }
}
fn validate(placement: &PlannedGear) -> Result<(), String> {
    let expected = conduit_std_offers::pcm_clip_frames_offer();
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract
        || !placement.host_calls.is_empty()
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("PCM clip frames placement differs from its exact pure profile".into());
    }
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: MAXIMUM_PCM_CLIP_BYTES as u32 + 2 * PCM_CLIP_FRAMES_MAXIMUM_OUTPUT_BYTES,
        host_requests: 0,
        sign_items: 2048,
        maximum_value_bytes: MAXIMUM_PCM_CLIP_BYTES as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::PcmClipFrames(PcmClipFramesBack::new()))
}

#[cfg(test)]
#[path = "pcm_clip_frames_back_tests.rs"]
mod tests;
