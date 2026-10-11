//! One Body execution path over the ordinary protocol kernel, with optional clock.
use crate::{
    i2c_base::I2cProvider,
    monotonic_clock::owner::MonotonicDeadlineProvider,
    protocol_host_calls::ProtocolCallRefusal,
    protocol_play::{PreparedProtocolPlay, PreparedTimedProtocolPlay},
};
use alloc::boxed::Box;
use conduit_composite::{KernelCompositeHost, KernelCompositeStatus};
use conduit_core::{PortId, ValuePayload};
use conduit_kernel::scheduler::RemoteIngressOutcome;

pub(super) enum ProtocolKernel<P, C> {
    WithClock(Box<PreparedTimedProtocolPlay<P, C>>),
    WithoutClock(Box<PreparedProtocolPlay<P>>),
}

impl<P: I2cProvider, C: MonotonicDeadlineProvider> ProtocolKernel<P, C> {
    pub(super) fn kernel(&self) -> &KernelCompositeHost {
        match self {
            Self::WithClock(kernel) => kernel.kernel(),
            Self::WithoutClock(kernel) => kernel.kernel(),
        }
    }
    pub(super) fn start(&mut self) -> Result<(), ProtocolCallRefusal> {
        match self {
            Self::WithClock(kernel) => kernel.start(),
            Self::WithoutClock(kernel) => kernel.start(),
        }
    }
    pub(super) fn step(&mut self) -> Result<KernelCompositeStatus, ProtocolCallRefusal> {
        match self {
            Self::WithClock(kernel) => kernel.step(),
            Self::WithoutClock(kernel) => kernel.step(),
        }
    }
    pub(super) fn pending_retirement_call(
        &self,
    ) -> Result<Option<super::retirement::PendingRetirementCall>, ProtocolCallRefusal> {
        match self {
            Self::WithClock(kernel) => kernel.pending_retirement_call(),
            Self::WithoutClock(kernel) => kernel.pending_retirement_call(),
        }
    }
    pub(super) fn cancel(&mut self) -> Result<(), ProtocolCallRefusal> {
        match self {
            Self::WithClock(kernel) => kernel.cancel(),
            Self::WithoutClock(kernel) => kernel.cancel(),
        }
    }
    pub(super) fn has_pending_clock(&self) -> bool {
        match self {
            Self::WithClock(kernel) => kernel.has_pending_clock(),
            Self::WithoutClock(_) => false,
        }
    }
    pub(super) fn admit_input(
        &mut self,
        port: &PortId,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<RemoteIngressOutcome, ProtocolCallRefusal> {
        match self {
            Self::WithClock(kernel) => kernel.admit_input(port, sequence, value),
            Self::WithoutClock(kernel) => kernel.admit_input(port, sequence, value),
        }
    }
    pub(super) fn close_input(&mut self, port: &PortId) -> Result<(), ProtocolCallRefusal> {
        match self {
            Self::WithClock(kernel) => kernel.close_input(port),
            Self::WithoutClock(kernel) => kernel.close_input(port),
        }
    }
    pub(super) fn output_into(
        &mut self,
        port: &PortId,
        output: &mut ValuePayload,
    ) -> Result<Option<u64>, ProtocolCallRefusal> {
        match self {
            Self::WithClock(kernel) => kernel.output_into(port, output),
            Self::WithoutClock(kernel) => kernel.output_into(port, output),
        }
    }
    pub(super) fn complete_output(
        &mut self,
        port: &PortId,
        sequence: u64,
    ) -> Result<(), ProtocolCallRefusal> {
        match self {
            Self::WithClock(kernel) => kernel.complete_output(port, sequence),
            Self::WithoutClock(kernel) => kernel.complete_output(port, sequence),
        }
    }
}
