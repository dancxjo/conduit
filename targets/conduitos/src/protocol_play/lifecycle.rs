//! Bounded execution and input/output lifecycle of an admitted protocol Play.
use super::{PreparedProtocolPlay, ProtocolCallRefusal};
use crate::i2c_base::I2cProvider;
use conduit_composite::{KernelCompositeHost, KernelCompositeStatus};
use conduit_core::{PortId, ValuePayload};

impl<P: I2cProvider> PreparedProtocolPlay<P> {
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    pub(crate) fn bind_storage_body(
        &mut self,
        pin: Option<crate::protocol_source::StorageBodyPreparation>,
    ) -> Result<(), ProtocolCallRefusal> {
        self.calls.bind_storage_body(pin)
    }

    pub fn start(&mut self) -> Result<(), ProtocolCallRefusal> {
        self.kernel
            .start()
            .map(|_| ())
            .map_err(ProtocolCallRefusal::Kernel)
    }

    pub fn admit_input(
        &mut self,
        port: &PortId,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<conduit_kernel::scheduler::RemoteIngressOutcome, ProtocolCallRefusal> {
        self.kernel
            .admit_input(port, sequence, value)
            .map_err(ProtocolCallRefusal::Kernel)
    }

    pub fn close_input(&mut self, port: &PortId) -> Result<(), ProtocolCallRefusal> {
        self.kernel
            .close_input(port)
            .map_err(ProtocolCallRefusal::Kernel)
    }

    pub fn output_into(
        &mut self,
        port: &PortId,
        output: &mut ValuePayload,
    ) -> Result<Option<u64>, ProtocolCallRefusal> {
        self.kernel
            .output_into(port, output)
            .map_err(ProtocolCallRefusal::Kernel)
    }

    pub fn complete_output(
        &mut self,
        port: &PortId,
        sequence: u64,
    ) -> Result<(), ProtocolCallRefusal> {
        self.kernel
            .complete_output(port, sequence)
            .map_err(ProtocolCallRefusal::Kernel)
    }

    /// Inspection cannot cancel or replace the admitted kernel behind the native owners.
    pub fn kernel(&self) -> &KernelCompositeHost {
        &self.kernel
    }

    /// Advance the sole kernel once and service at most one surfaced Host Call.
    pub fn step(&mut self) -> Result<KernelCompositeStatus, ProtocolCallRefusal> {
        let status = self.kernel.step().map_err(ProtocolCallRefusal::Kernel)?;
        if self.calls.poll_pending(&mut self.kernel)? {
            return Ok(status);
        }
        let request = self
            .deferred_host_request
            .take()
            .or_else(|| self.kernel.next_host_request());
        if let Some(request) = request {
            if self.calls.can_dispatch(&self.kernel, &request)? {
                self.calls.dispatch(&mut self.kernel, &request)?;
            } else {
                self.deferred_host_request = Some(request);
            }
        }
        Ok(status)
    }

    pub fn cancel(&mut self) -> Result<(), ProtocolCallRefusal> {
        self.calls.cancel(&mut self.kernel)
    }
}
