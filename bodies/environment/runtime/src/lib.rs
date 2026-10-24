#![no_std]
//! Shared kernel Backs for the environmental Source composition.
extern crate alloc;
#[cfg(feature = "appliance")]
pub mod appliance;
pub mod assigned;
mod canonical_input;
mod canonical_storage;
#[cfg(feature = "construction")]
pub mod construction;
pub mod display_packets;
#[cfg(feature = "display-output")]
pub mod display_output;
pub mod observation;
pub mod publication;
#[cfg(feature = "face-projection")]
pub mod publication_face;
pub mod recording;
use alloc::{boxed::Box, string::String};
use conduit_core::PortTemporal;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId,
};
use conduit_plot::PortableExpressionProgram;
use conduit_semantic_catalog::{PreparedExpressionBack, StructuredSelectorBack};

/// The ordinary production Backs retain their lifecycle and commit ownership.
/// Targets service their exact admitted Host Calls; this is no scheduler.
pub enum CaptureBack {
    Expression(Box<PreparedExpressionBack>),
    Selector(StructuredSelectorBack),
    Pulse(conduit_kernel::scheduler::HostCallBack),
    Observation(Box<observation::ObservationBack>),
}
impl CaptureBack {
    pub fn observation(
        contract: &observation::ObservationContract,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        Ok(Self::Observation(Box::new(
            observation::ObservationBack::prepare(contract)?,
        )))
    }
    pub fn expression(
        program: &PortableExpressionProgram,
        temporal: PortTemporal,
    ) -> Result<Self, String> {
        Ok(Self::Expression(Box::new(
            PreparedExpressionBack::prepare_with_temporal(program, temporal)?,
        )))
    }
    pub fn selector(maximum_input_bytes: u32) -> Self {
        Self::Selector(StructuredSelectorBack::new(maximum_input_bytes))
    }
    pub fn pulse(maximum_input_bytes: u32) -> Self {
        Self::Pulse(conduit_kernel::scheduler::HostCallBack::new(
            maximum_input_bytes,
        ))
    }
}
impl<const P: usize> StepBack<P> for CaptureBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        match self {
            Self::Expression(back) => back.step(io, input),
            Self::Selector(back) => back.step(io, input),
            Self::Pulse(back) => back.step(io, input),
            Self::Observation(back) => back.step(io, input),
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Expression(back) => {
                <PreparedExpressionBack as StepBack<P>>::prepared_output(back, port)
            }
            Self::Observation(back) => {
                <observation::ObservationBack as StepBack<P>>::prepared_output(back, port)
            }
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        if let Self::Expression(back) = self {
            <PreparedExpressionBack as StepBack<P>>::step_committed(back)
        }
        if let Self::Observation(back) = self {
            <observation::ObservationBack as StepBack<P>>::step_committed(back);
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Observation(back) => <observation::ObservationBack as StepBack<P>>::cancel(back),
            Self::Expression(back) => <PreparedExpressionBack as StepBack<P>>::cancel(back),
            Self::Selector(back) => <StructuredSelectorBack as StepBack<P>>::cancel(back),
            Self::Pulse(back) => {
                <conduit_kernel::scheduler::HostCallBack as StepBack<P>>::cancel(back)
            }
        }
    }
}

mod capture_selector;
pub use capture_selector::PreparedCaptureSelector;
