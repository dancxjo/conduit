//! Exact kernel-token dispatch and cooperative native completion.
#[cfg(target_arch = "x86_64")]
use super::PendingUsbKind;
use super::{I2C_CALL, I2cProvider, Owner, PreparedProtocolCalls, ProtocolCallRefusal};
use conduit_composite::{
    AdmittedKernelCompositeHostRequest, KernelCompositeHost, KernelCompositeHostRequest,
};
use conduit_kernel::{FailureCode, HostCallDisposition, HostCallId, HostCallOutcome};

impl<P: I2cProvider> PreparedProtocolCalls<P> {
    /// Service exactly one surfaced token. No request creation, retry or private stepping loop.
    pub fn dispatch(
        &mut self,
        kernel: &mut KernelCompositeHost,
        request: &KernelCompositeHostRequest,
    ) -> Result<(), ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        if kernel.definition().internal_plan.plan_id != self.plan_id {
            return Err(Refusal::InvalidPlan);
        }
        #[cfg(target_arch = "x86_64")]
        if self.pending_usb.is_some() {
            return Err(Refusal::Unsupported);
        }
        let view = kernel.host_request_view(request).map_err(Refusal::Kernel)?;
        if *view.child != self.identity.host_id || view.request.call != HostCallId(0) {
            return Err(Refusal::InvalidPlan);
        }
        let node = view.request.node;
        let binding = self
            .bindings
            .iter_mut()
            .find(|binding| binding.node == node)
            .ok_or(Refusal::Unsupported)?;
        let required = kernel
            .host_request_obligation(request)
            .map_err(Refusal::Kernel)?;
        let expected = match binding.owner {
            Owner::Pure(ref owner) => owner.contract(),
            Owner::I2c(_) => I2C_CALL,
            #[cfg(target_arch = "x86_64")]
            Owner::UsbControl(_) | Owner::UsbProtocolControl | Owner::UsbDuplexControl => {
                crate::usb_base::control_contract::CONTROL_CALL
            }
            #[cfg(target_arch = "x86_64")]
            Owner::UsbProtocolEndpoint | Owner::UsbDuplexRead => {
                crate::usb_base::endpoint_read_contract::ENDPOINT_READ_CALL
            }
            #[cfg(target_arch = "x86_64")]
            Owner::UsbDuplexWrite => crate::usb_base::endpoint_write_contract::ENDPOINT_WRITE_CALL,
        };
        if required.requirement.contract_id.as_str() != expected {
            return Err(Refusal::InvalidPlan);
        }
        let admitted = kernel
            .admit_host_request(
                request,
                &self.identity,
                &binding.resources,
                &binding.authorities,
            )
            .map_err(Refusal::Kernel)?;
        let call = *kernel
            .admitted_host_request_view(&admitted)
            .map_err(Refusal::Kernel)?
            .request;
        let input = kernel
            .host_request_input(&admitted)
            .map_err(Refusal::Kernel)?;
        #[cfg(target_arch = "x86_64")]
        if matches!(
            binding.owner,
            Owner::UsbProtocolEndpoint | Owner::UsbDuplexRead | Owner::UsbDuplexWrite
        ) {
            let (kind, result) = match binding.owner {
                Owner::UsbProtocolEndpoint => (
                    PendingUsbKind::ProtocolRead,
                    self.usb_protocol
                        .as_mut()
                        .ok_or(Refusal::InvalidPlan)?
                        .endpoint_begin(node, call.call, call.request, input),
                ),
                Owner::UsbDuplexRead => (
                    PendingUsbKind::DuplexRead,
                    self.usb_duplex
                        .as_mut()
                        .ok_or(Refusal::InvalidPlan)?
                        .read_begin(node, call.call, call.request, input),
                ),
                Owner::UsbDuplexWrite => (
                    PendingUsbKind::DuplexWrite,
                    self.usb_duplex
                        .as_mut()
                        .ok_or(Refusal::InvalidPlan)?
                        .write_begin(node, call.call, call.request, input),
                ),
                _ => return Err(Refusal::InvalidPlan),
            };
            let result = result.map_err(Refusal::UsbProtocol);
            match result {
                Ok(()) => {
                    self.pending_usb_kind = kind;
                    self.pending_usb = Some(admitted);
                    return Ok(());
                }
                Err(error) => {
                    Self::complete_failure(kernel, &admitted, &error)?;
                    return Err(error);
                }
            }
        }
        let result = match &mut binding.owner {
            #[cfg(target_arch = "x86_64")]
            Owner::UsbProtocolControl => self
                .usb_protocol
                .as_mut()
                .ok_or(Refusal::InvalidPlan)?
                .control_invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::UsbProtocol),
            #[cfg(target_arch = "x86_64")]
            Owner::UsbProtocolEndpoint | Owner::UsbDuplexRead | Owner::UsbDuplexWrite => {
                return Err(Refusal::InvalidPlan);
            }
            #[cfg(target_arch = "x86_64")]
            Owner::UsbDuplexControl => self
                .usb_duplex
                .as_mut()
                .ok_or(Refusal::InvalidPlan)?
                .control_invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::UsbProtocol),
            #[cfg(target_arch = "x86_64")]
            Owner::UsbControl(owner) => owner
                .invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::Usb),
            Owner::Pure(owner) => owner.invoke(node, call.call, call.request, input),
            Owner::I2c(owner) => owner
                .invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::I2c),
        };
        match result {
            Ok(None) => kernel
                .complete_host_call(
                    &admitted,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .map_err(Refusal::Kernel),
            Ok(Some(bytes)) => kernel
                .complete_host_call_bytes(&admitted, bytes)
                .map_err(Refusal::Kernel),
            Err(error) => {
                let failure = error.failure();
                kernel
                    .complete_host_call(
                        &admitted,
                        HostCallOutcome {
                            disposition: if failure.code == FailureCode::HostCallDenied {
                                HostCallDisposition::Denied
                            } else {
                                HostCallDisposition::Failed
                            },
                            output: None,
                            failure: Some(failure),
                        },
                    )
                    .map_err(Refusal::Kernel)?;
                Err(error)
            }
        }
    }

    fn complete_failure(
        kernel: &mut KernelCompositeHost,
        admitted: &AdmittedKernelCompositeHostRequest,
        error: &ProtocolCallRefusal,
    ) -> Result<(), ProtocolCallRefusal> {
        let failure = error.failure();
        kernel
            .complete_host_call(
                admitted,
                HostCallOutcome {
                    disposition: if failure.code == FailureCode::HostCallDenied {
                        HostCallDisposition::Denied
                    } else {
                        HostCallDisposition::Failed
                    },
                    output: None,
                    failure: Some(failure),
                },
            )
            .map_err(ProtocolCallRefusal::Kernel)
    }

    /// Poll at most one native event for the original admitted endpoint token.
    pub fn poll_pending(
        &mut self,
        kernel: &mut KernelCompositeHost,
    ) -> Result<bool, ProtocolCallRefusal> {
        if kernel.definition().internal_plan.plan_id != self.plan_id {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        #[cfg(target_arch = "x86_64")]
        if let Some(admitted) = self.pending_usb {
            if self.pending_usb_failed {
                return Err(ProtocolCallRefusal::Unsupported);
            }
            let result = match self.pending_usb_kind {
                PendingUsbKind::ProtocolRead => self
                    .usb_protocol
                    .as_mut()
                    .ok_or(ProtocolCallRefusal::InvalidPlan)?
                    .endpoint_poll(),
                PendingUsbKind::DuplexRead => self
                    .usb_duplex
                    .as_mut()
                    .ok_or(ProtocolCallRefusal::InvalidPlan)?
                    .read_poll(),
                PendingUsbKind::DuplexWrite => self
                    .usb_duplex
                    .as_mut()
                    .ok_or(ProtocolCallRefusal::InvalidPlan)?
                    .write_poll(),
            }
            .map_err(ProtocolCallRefusal::UsbProtocol);
            match result {
                Ok(None) => {}
                Ok(Some(bytes)) => {
                    kernel
                        .complete_host_call_bytes(&admitted, bytes)
                        .map_err(ProtocolCallRefusal::Kernel)?;
                    self.pending_usb = None;
                }
                Err(error) => {
                    self.pending_usb_failed = true;
                    Self::complete_failure(kernel, &admitted, &error)?;
                    return Err(error);
                }
            }
            return Ok(true);
        }
        Ok(false)
    }
}
