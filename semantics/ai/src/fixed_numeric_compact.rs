//! Compact pointwise owners and shared finite-f32 transport helpers.
//! Arithmetic and commit state stay in the canonical owners. Only a kernel-bound
//! internal output uses raw lanes; every escaping output retains its exact codec.
use crate::{
    fixed_numeric_codec::{FixedCodecRefusal, FixedF32VectorCodec},
    fixed_numeric_operations_back::FixedTanhBack,
    fixed_numeric_signal_back::FixedElementwiseBack,
};
use alloc::{vec, vec::Vec};
use conduit_kernel::{
    scheduler::{
        prepared_session::ReusableStepBack, CompactF32Layout, SchedulerError, StepBack,
        StepInputBytes, StepIo, StepOutcome,
    },
    PortId, ValueStorage,
};

/// Selected physical profiles; both retain every logical node and Cord.
pub const COMPACT_DAG_PROFILE: &str = "conduit.numeric/finite-f32-dag@1";
pub const CANONICAL_DAG_PROFILE: &str = "conduit.numeric/canonical-f32-dag@1";
pub const COMPACT_DAG_IMPLEMENTATION: &str = "conduit.numeric/compact-pointwise-dag@1";
pub const CANONICAL_DAG_IMPLEMENTATION: &str = "conduit.numeric/canonical-pointwise-dag@1";
pub const DAG_ARTIFACT: &str = "conduit-kernel/logical-step-f32-dag@1";

pub(crate) struct CompactVectorBuffer<const W: usize> {
    bytes: Vec<u8>,
    published: bool,
}

impl<const W: usize> CompactVectorBuffer<W> {
    pub(crate) fn prepare() -> Result<Self, FixedCodecRefusal> {
        if W == 0 || W > 1024 {
            return Err(FixedCodecRefusal::Shape);
        }
        Ok(Self {
            bytes: vec![0; W * 4],
            published: false,
        })
    }
    pub(crate) fn reset(&mut self) {
        self.published = false;
        // A committed ownership transfer returns an empty, fully reserved buffer.
        self.bytes.resize(W * 4, 0);
    }
    pub(crate) fn published_bytes(&self) -> Option<&[u8]> {
        self.published.then_some(self.bytes.as_slice())
    }
    pub(crate) fn published_buffer_mut(&mut self) -> Option<&mut Vec<u8>> {
        self.published.then_some(&mut self.bytes)
    }
    pub(crate) fn scratch_bytes(&self) -> usize {
        self.bytes.capacity()
    }
}

pub(crate) fn decode<const W: usize, const P: usize>(
    codec: &FixedF32VectorCodec<W>,
    inputs: &StepInputBytes<'_, P>,
    port: PortId,
    output: &mut [f32; W],
    admitted: bool,
) -> Result<(), FixedCodecRefusal> {
    if let Some(bytes) = admitted.then(|| inputs.compact_f32(port)).flatten() {
        if bytes.len() != W * 4 {
            return Err(FixedCodecRefusal::Encoding);
        }
        let mut staged = [0.; W];
        for (value, bytes) in staged.iter_mut().zip(bytes.as_chunks::<4>().0.iter()) {
            *value = f32::from_le_bytes(*bytes);
            if !value.is_finite() {
                return Err(FixedCodecRefusal::Nonfinite);
            }
        }
        *output = staged;
        Ok(())
    } else {
        codec.decode(
            inputs.input(port).ok_or(FixedCodecRefusal::Encoding)?,
            output,
        )
    }
}

pub(crate) fn encode<const W: usize, const P: usize>(
    codec: &mut FixedF32VectorCodec<W>,
    values: &[f32; W],
    io: &StepIo<P>,
    compact: Option<&mut CompactVectorBuffer<W>>,
) -> Result<(), FixedCodecRefusal> {
    if let Some(buffer) = compact.filter(|_| io.compact_f32_output(PortId(0)).is_some()) {
        let expected = CompactF32Layout {
            lanes: u16::try_from(W).map_err(|_| FixedCodecRefusal::Shape)?,
            logical_bytes: u32::try_from(codec.maximum_bytes())
                .map_err(|_| FixedCodecRefusal::Shape)?,
        };
        if io.compact_f32_output(PortId(0)) != Some(expected) {
            return Err(FixedCodecRefusal::Shape);
        }
        // Preserve canonical finite-result rejection before mutating any output.
        if values.iter().any(|v| !v.is_finite()) {
            return Err(FixedCodecRefusal::Nonfinite);
        }
        for (bytes, value) in buffer.bytes.as_chunks_mut::<4>().0.iter_mut().zip(values) {
            bytes.copy_from_slice(&value.to_le_bytes());
        }
        buffer.published = true;
        Ok(())
    } else {
        codec.encode(values).map(|_| ())
    }
}

pub(crate) fn send<const W: usize, const P: usize>(
    io: &mut StepIo<P>,
    logical_bytes: usize,
    compact: Option<&CompactVectorBuffer<W>>,
) -> Result<(), SchedulerError> {
    if compact.is_some_and(|buffer| buffer.published) {
        io.send_compact_f32(PortId(0))
    } else {
        io.send_prepared(PortId(0), logical_bytes as u32)
    }
}

enum Owner<const W: usize> {
    Elementwise(FixedElementwiseBack<W>),
    Tanh(FixedTanhBack<W>),
}
/// Distinct owner with no canonical `output()` accessor. Native intermediate
/// publication never updates or exposes a stale canonical output as a result.
pub struct CompactNumericBack<const W: usize> {
    owner: Owner<W>,
    buffer: CompactVectorBuffer<W>,
    arity: usize,
    layout: CompactF32Layout,
}
impl<const W: usize> CompactNumericBack<W> {
    pub fn from_elementwise(owner: FixedElementwiseBack<W>) -> Result<Self, FixedCodecRefusal> {
        let shape = owner.compact_shape().ok_or(FixedCodecRefusal::Shape)?;
        Self::prepare(Owner::Elementwise(owner), shape)
    }
    pub fn from_tanh(owner: FixedTanhBack<W>) -> Result<Self, FixedCodecRefusal> {
        let shape = owner.compact_shape().ok_or(FixedCodecRefusal::Shape)?;
        Self::prepare(Owner::Tanh(owner), shape)
    }
    fn prepare(owner: Owner<W>, (arity, bytes): (usize, usize)) -> Result<Self, FixedCodecRefusal> {
        if W == 0 || W > 1024 || W * 4 > bytes {
            return Err(FixedCodecRefusal::Shape);
        }
        Ok(Self {
            owner,
            buffer: CompactVectorBuffer {
                bytes: vec![0; W * 4],
                published: false,
            },
            arity,
            layout: CompactF32Layout {
                lanes: u16::try_from(W).map_err(|_| FixedCodecRefusal::Shape)?,
                logical_bytes: u32::try_from(bytes).map_err(|_| FixedCodecRefusal::Shape)?,
            },
        })
    }
    /// Additional raw scratch, allocated once during preparation.
    pub fn compact_scratch_bytes(&self) -> usize {
        self.buffer.bytes.capacity()
    }
}
impl<const W: usize, const P: usize> StepBack<P> for CompactNumericBack<W> {
    fn compact_f32_input(&self, port: PortId) -> Option<CompactF32Layout> {
        (usize::from(port.0) < self.arity).then_some(self.layout)
    }
    fn compact_f32_output(&self) -> Option<(PortId, CompactF32Layout)> {
        Some((PortId(0), self.layout))
    }
    fn owns_compact_f32_output(&self, port: PortId) -> bool {
        port == PortId(0)
    }
    fn prepared_compact_f32_output_mut(&mut self, port: PortId) -> Option<&mut Vec<u8>> {
        (port == PortId(0))
            .then(|| self.buffer.published_buffer_mut())
            .flatten()
    }
    fn step(&mut self, io: &mut StepIo<P>, inputs: &StepInputBytes<'_, P>) -> StepOutcome {
        self.buffer.reset();
        match &mut self.owner {
            Owner::Elementwise(back) => back.step_transport(io, inputs, Some(&mut self.buffer)),
            Owner::Tanh(back) => back.step_transport(io, inputs, Some(&mut self.buffer)),
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if port == PortId(0) && self.buffer.published {
            Some(&self.buffer.bytes)
        } else {
            match &self.owner {
                Owner::Elementwise(back) => {
                    <FixedElementwiseBack<W> as StepBack<P>>::prepared_output(back, port)
                }
                Owner::Tanh(back) => <FixedTanhBack<W> as StepBack<P>>::prepared_output(back, port),
            }
        }
    }
    fn step_committed(&mut self) {
        match &mut self.owner {
            Owner::Elementwise(back) => {
                <FixedElementwiseBack<W> as StepBack<P>>::step_committed(back)
            }
            Owner::Tanh(back) => <FixedTanhBack<W> as StepBack<P>>::step_committed(back),
        }
    }
    fn cancel(&mut self) {
        self.buffer.published = false;
        match &mut self.owner {
            Owner::Elementwise(back) => <FixedElementwiseBack<W> as StepBack<P>>::cancel(back),
            Owner::Tanh(back) => <FixedTanhBack<W> as StepBack<P>>::cancel(back),
        }
    }
}
impl<const W: usize, const P: usize> ReusableStepBack<P> for CompactNumericBack<W> {
    type Next = ();
    fn preflight_next(&self, next: &(), storage: &dyn ValueStorage) -> bool {
        match &self.owner {
            Owner::Elementwise(back) => {
                <FixedElementwiseBack<W> as ReusableStepBack<P>>::preflight_next(
                    back, next, storage,
                )
            }
            Owner::Tanh(back) => {
                <FixedTanhBack<W> as ReusableStepBack<P>>::preflight_next(back, next, storage)
            }
        }
    }
    fn begin_next(&mut self, next: ()) {
        self.buffer.published = false;
        match &mut self.owner {
            Owner::Elementwise(back) => {
                <FixedElementwiseBack<W> as ReusableStepBack<P>>::begin_next(back, next)
            }
            Owner::Tanh(back) => <FixedTanhBack<W> as ReusableStepBack<P>>::begin_next(back, next),
        }
    }
}
