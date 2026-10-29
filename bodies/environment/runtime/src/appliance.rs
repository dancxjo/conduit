//! Portable production driver set for the authored environmental appliance.
//! All scheduling, routing, commit and Host Call state remain in the kernel.
use crate::{
    publication::{
        PublicationBack, PublicationContract, PublicationSequence, PublicationSnapshot,
        SourcePublicationBack,
    },
    CaptureBack,
};
use alloc::boxed::Box;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId,
};

pub mod assigned;
pub mod assignment_transfer;

pub enum ApplianceBack {
    Native(CaptureBack),
    Tick(conduit_time::TickBack),
    Trigger(conduit_time::TickTriggerBack),
    Publication(Box<PublicationBack>),
    SourcePublication(Box<SourcePublicationBack>),
}
impl ApplianceBack {
    /// Exact linked-Type profile; only immutable framing leaves resident RAM.
    pub fn source_publication(
        contract: &PublicationContract,
        maximum_events: u32,
        sequence: PublicationSequence,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        Ok(Self::SourcePublication(Box::new(
            SourcePublicationBack::prepare(contract, maximum_events, sequence)?,
        )))
    }
    pub fn publication_sequence(&self) -> Option<u64> {
        match self {
            Self::Publication(back) => back.snapshot().map(|s| s.sequence),
            Self::SourcePublication(back) => (back.sequence() != 0).then_some(back.sequence()),
            _ => None,
        }
    }
    /// The callback finishes before any reconstructed canonical scratch expires.
    pub fn with_publication_snapshot<R>(
        &self,
        consume: impl FnOnce(PublicationSnapshot<'_>) -> R,
    ) -> Option<R> {
        match self {
            Self::Publication(back) => back.snapshot().map(consume),
            Self::SourcePublication(back) => back.with_snapshot(consume),
            _ => None,
        }
    }
    /// Box only during admission; spare enum capacity is not replicated across
    /// every expression and selector node in an MCU's fixed driver table.
    pub fn publication(
        contract: &PublicationContract,
        maximum_events: u32,
        sequence: PublicationSequence,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        Ok(Self::Publication(Box::new(PublicationBack::prepare(
            contract,
            maximum_events,
            sequence,
        )?)))
    }
}
impl<const P: usize> StepBack<P> for ApplianceBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        match self {
            Self::Native(b) => b.step(io, input),
            Self::Tick(b) => b.step(io, input),
            Self::Trigger(b) => b.step(io, input),
            Self::Publication(b) => b.step(io, input),
            Self::SourcePublication(b) => b.step(io, input),
        }
    }
    fn encodes_prepared_output(&self, port: PortId) -> bool {
        match self {
            Self::Native(back) => <CaptureBack as StepBack<P>>::encodes_prepared_output(back, port),
            _ => false,
        }
    }
    fn encode_prepared_output(
        &self,
        port: PortId,
        output: &mut [u8],
    ) -> Result<usize, conduit_kernel::StorageError> {
        match self {
            Self::Native(back) => {
                <CaptureBack as StepBack<P>>::encode_prepared_output(back, port, output)
            }
            _ => {
                let bytes = <Self as StepBack<P>>::prepared_output(self, port)
                    .ok_or(conduit_kernel::StorageError::RepresentationMismatch)?;
                output
                    .get_mut(..bytes.len())
                    .ok_or(conduit_kernel::StorageError::ValueTooLarge)?
                    .copy_from_slice(bytes);
                Ok(bytes.len())
            }
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Native(b) => <CaptureBack as StepBack<P>>::prepared_output(b, port),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Native(b) => <CaptureBack as StepBack<P>>::step_committed(b),
            Self::Tick(b) => <conduit_time::TickBack as StepBack<P>>::step_committed(b),
            Self::Trigger(b) => <conduit_time::TickTriggerBack as StepBack<P>>::step_committed(b),
            Self::Publication(b) => <PublicationBack as StepBack<P>>::step_committed(b),
            Self::SourcePublication(b) => <SourcePublicationBack as StepBack<P>>::step_committed(b),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Native(b) => <CaptureBack as StepBack<P>>::cancel(b),
            Self::Tick(b) => <conduit_time::TickBack as StepBack<P>>::cancel(b),
            Self::Trigger(b) => <conduit_time::TickTriggerBack as StepBack<P>>::cancel(b),
            Self::Publication(b) => <PublicationBack as StepBack<P>>::cancel(b),
            Self::SourcePublication(b) => <SourcePublicationBack as StepBack<P>>::cancel(b),
        }
    }
}

#[cfg(feature = "construction")]
pub mod software;
#[cfg(feature = "construction")]
pub mod software_compact;

mod value_storage;
pub use value_storage::{prepare_value_storage, value_storage_bytes};
