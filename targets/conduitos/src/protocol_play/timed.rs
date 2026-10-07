//! One kernel with independent retained I2C and monotonic-clock owners.
use super::*;
use crate::monotonic_clock::{
    dispatch::PreparedClockDispatch, installation::ReadyClockBase, owner::MonotonicDeadlineProvider,
};

pub struct ClockAdmission<C> {
    pub ready: ReadyClockBase<C>,
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
}

pub struct PreparedTimedProtocolPlay<P, C> {
    play: PreparedProtocolPlay<P>,
    clock: PreparedClockDispatch<C>,
}
impl<P: I2cProvider, C: MonotonicDeadlineProvider> PreparedTimedProtocolPlay<P, C> {
    pub fn prepare(
        definition: KernelCompositeDefinition,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        operations: crate::protocol_operations::ProtocolOperations,
        clock: ClockAdmission<C>,
    ) -> Result<Self, ProtocolCallRefusal> {
        validate_fore(&definition)?;
        let clock = PreparedClockDispatch::prepare(
            &definition.internal_plan,
            clock.ready,
            clock.table,
            clock.handle,
            clock.claim,
        )?;
        let play = PreparedProtocolPlay::prepare_mode(
            definition, ready, table, handle, claim, operations, true,
        )?;
        Ok(Self { play, clock })
    }
    pub fn start(&mut self) -> Result<(), ProtocolCallRefusal> {
        self.play.start()
    }
    /// Advance the only kernel once, then service one admitted Host Call quantum.
    pub fn step(&mut self) -> Result<KernelCompositeStatus, ProtocolCallRefusal> {
        let status = self
            .play
            .kernel
            .step()
            .map_err(ProtocolCallRefusal::Kernel)?;
        if self.clock.has_pending() {
            self.clock.poll(&mut self.play.kernel)?;
        } else if let Some(request) = self.play.kernel.next_host_request() {
            if self.clock.owns_request(&self.play.kernel, &request)? {
                self.clock.dispatch(&mut self.play.kernel, &request)?;
            } else {
                self.play.calls.dispatch(&mut self.play.kernel, &request)?;
            }
        }
        Ok(status)
    }
    /// The containing machine may idle between quanta while this owner awaits
    /// a real deadline. This reports retained state and chooses no scheduling policy.
    pub fn has_pending_clock(&self) -> bool {
        self.clock.has_pending()
    }

    pub fn monotonic_observation(&mut self) -> Result<MonotonicInstant, ProtocolCallRefusal> {
        self.clock.monotonic_observation(&self.play.kernel)
    }

    pub fn cancel(&mut self) -> Result<(), ProtocolCallRefusal> {
        self.clock.revoke(&self.play.kernel)?;
        self.play.cancel()
    }
    pub fn admit_input(
        &mut self,
        port: &PortId,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<conduit_kernel::scheduler::RemoteIngressOutcome, ProtocolCallRefusal> {
        self.play.admit_input(port, sequence, value)
    }
    pub fn close_input(&mut self, port: &PortId) -> Result<(), ProtocolCallRefusal> {
        self.play.close_input(port)
    }
    pub fn output_into(
        &mut self,
        port: &PortId,
        output: &mut ValuePayload,
    ) -> Result<Option<u64>, ProtocolCallRefusal> {
        self.play.output_into(port, output)
    }
    pub fn complete_output(
        &mut self,
        port: &PortId,
        sequence: u64,
    ) -> Result<(), ProtocolCallRefusal> {
        self.play.complete_output(port, sequence)
    }
    pub fn kernel(&self) -> &KernelCompositeHost {
        self.play.kernel()
    }
}
