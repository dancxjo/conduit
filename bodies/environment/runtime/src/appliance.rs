//! Portable production driver set for the authored environmental appliance.
//! All scheduling, routing, commit and Host Call state remain in the kernel.
use crate::{
    publication::{PublicationBack, PublicationContract},
    CaptureBack,
};
use alloc::boxed::Box;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId,
};

pub enum ApplianceBack {
    Native(CaptureBack),
    Tick(conduit_time::TickBack),
    Trigger(conduit_time::TickTriggerBack),
    Publication(Box<PublicationBack>),
}
impl ApplianceBack {
    /// Box only during admission; spare enum capacity is not replicated across
    /// every expression and selector node in an MCU's fixed driver table.
    pub fn publication(
        contract: &PublicationContract,
        maximum_events: u32,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        Ok(Self::Publication(Box::new(PublicationBack::prepare(
            contract,
            maximum_events,
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
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Native(b) => <CaptureBack as StepBack<P>>::cancel(b),
            Self::Tick(b) => <conduit_time::TickBack as StepBack<P>>::cancel(b),
            Self::Trigger(b) => <conduit_time::TickTriggerBack as StepBack<P>>::cancel(b),
            Self::Publication(b) => <PublicationBack as StepBack<P>>::cancel(b),
        }
    }
}
