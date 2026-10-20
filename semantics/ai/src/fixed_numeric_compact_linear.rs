//! Compact numeric transport around the existing admitted tensor projection.
use crate::{
    fixed_numeric_codec::FixedCodecRefusal, fixed_numeric_compact::CompactVectorBuffer,
    fixed_numeric_linear_back::FixedLinearBack,
};
use conduit_core::AdmittedResourceAccess;
use conduit_kernel::{
    scheduler::{
        prepared_session::ReusableStepBack, CompactF32Layout, StepBack, StepInputBytes, StepIo,
        StepOutcome,
    },
    PortId, ValueStorage,
};

/// Retains the exact admitted resource and arithmetic owner. The resource port
/// stays canonical and is checked on every invocation. Only an explicitly bound
/// internal numeric output uses raw lanes; escaping outputs keep their codec.
/// No canonical output accessor is exposed for a raw intermediate publication.
pub struct CompactLinearBack<'a, const I: usize, const O: usize> {
    owner: FixedLinearBack<'a, I, O>,
    buffer: CompactVectorBuffer<O>,
    input: CompactF32Layout,
    output: CompactF32Layout,
}
impl<'a, const I: usize, const O: usize> CompactLinearBack<'a, I, O> {
    pub fn from_linear(owner: FixedLinearBack<'a, I, O>) -> Result<Self, FixedCodecRefusal> {
        let (input, output) = owner.compact_layouts()?;
        Ok(Self {
            owner,
            buffer: CompactVectorBuffer::prepare()?,
            input,
            output,
        })
    }
    pub fn admitted_access(&self) -> &AdmittedResourceAccess {
        self.owner.admitted_access()
    }
    pub fn has_committed_result(&self) -> bool {
        self.owner.has_committed_result()
    }
    /// Additional raw output backing, allocated once during preparation.
    pub fn compact_scratch_bytes(&self) -> usize {
        self.buffer.scratch_bytes()
    }
}
impl<const I: usize, const O: usize, const P: usize> StepBack<P> for CompactLinearBack<'_, I, O> {
    fn compact_f32_input(&self, port: PortId) -> Option<CompactF32Layout> {
        (port == PortId(0)).then_some(self.input)
    }
    fn compact_f32_output(&self) -> Option<(PortId, CompactF32Layout)> {
        Some((PortId(0), self.output))
    }
    fn step(&mut self, io: &mut StepIo<P>, inputs: &StepInputBytes<'_, P>) -> StepOutcome {
        self.buffer.reset();
        self.owner
            .step_transport(io, inputs, Some(&mut self.buffer))
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if port == PortId(0) {
            self.buffer.published_bytes().or_else(|| {
                <FixedLinearBack<I, O> as StepBack<P>>::prepared_output(&self.owner, port)
            })
        } else {
            None
        }
    }
    fn step_committed(&mut self) {
        <FixedLinearBack<I, O> as StepBack<P>>::step_committed(&mut self.owner);
    }
    fn cancel(&mut self) {
        self.buffer.reset();
        <FixedLinearBack<I, O> as StepBack<P>>::cancel(&mut self.owner);
    }
}
impl<const I: usize, const O: usize, const P: usize> ReusableStepBack<P>
    for CompactLinearBack<'_, I, O>
{
    type Next = ();
    fn preflight_next(&self, next: &(), storage: &dyn ValueStorage) -> bool {
        <FixedLinearBack<I, O> as ReusableStepBack<P>>::preflight_next(&self.owner, next, storage)
    }
    fn begin_next(&mut self, next: ()) {
        self.buffer.reset();
        <FixedLinearBack<I, O> as ReusableStepBack<P>>::begin_next(&mut self.owner, next);
    }
}
