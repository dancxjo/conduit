//! Exact kernel-token dispatch and cooperative native completion.
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-board"))
))]
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
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-board"))
        ))]
        if self.pending_usb.is_some() {
            return Err(Refusal::Unsupported);
        }
        if !self.can_dispatch(kernel, request)? {
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
            Owner::Spi(_) => crate::spi_base::contract::SPI_CALL,
            Owner::Digital(_) => crate::digital_base::contract::DIGITAL_CALL,
            #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
            Owner::EthernetFrame(_) => crate::ethernet_frame::contract::FRAME_CALL,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::StorageAuthorize => crate::storage_base::transaction_contract::AUTHORIZE_CALL,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::StorageSettle => crate::storage_base::transaction_contract::SETTLE_CALL,

            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::UsbControl(_)
            | Owner::UsbProtocolControl
            | Owner::UsbDuplexControl
            | Owner::UsbIsochronousControl => crate::usb_base::control_contract::CONTROL_CALL,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::UsbProtocolEndpoint | Owner::UsbDuplexRead => {
                crate::usb_base::endpoint_read_contract::ENDPOINT_READ_CALL
            }
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::UsbDuplexWrite => crate::usb_base::endpoint_write_contract::ENDPOINT_WRITE_CALL,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::UsbIsochronousOut => {
                crate::usb_base::isochronous::contract::ISOCHRONOUS_OUT_CALL
            }
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
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-board"))
        ))]
        if matches!(
            binding.owner,
            Owner::StorageAuthorize | Owner::StorageSettle
        ) {
            let settle = matches!(binding.owner, Owner::StorageSettle);
            let result =
                self.dispatch_storage(kernel, &admitted, node, call.call, call.request, settle);
            if let Err(error) = result {
                Self::complete_failure(kernel, &admitted, &error)?;
                return Err(error);
            }
            return Ok(());
        }
        let input = kernel
            .host_request_input(&admitted)
            .map_err(Refusal::Kernel)?;
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-board"))
        ))]
        if matches!(binding.owner, Owner::UsbIsochronousOut) {
            let result = self
                .usb_isochronous
                .as_mut()
                .ok_or(Refusal::InvalidPlan)?
                .isochronous_begin(node, call.call, call.request, input)
                .map_err(Refusal::UsbIsochronous);
            match result {
                Ok(()) => {
                    self.pending_usb_kind = PendingUsbKind::IsochronousOut;
                    self.pending_usb = Some(admitted);
                    return Ok(());
                }
                Err(error) => {
                    Self::complete_failure(kernel, &admitted, &error)?;
                    return Err(error);
                }
            }
        }
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-board"))
        ))]
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
                    if matches!(
                        kind,
                        PendingUsbKind::DuplexRead | PendingUsbKind::DuplexWrite
                    ) {
                        let direction = usize::from(matches!(kind, PendingUsbKind::DuplexWrite));
                        self.pending_duplex[direction] = Some((node, admitted));
                    } else {
                        self.pending_usb_kind = kind;
                        self.pending_usb = Some(admitted);
                    }
                    return Ok(());
                }
                Err(error) => {
                    Self::complete_failure(kernel, &admitted, &error)?;
                    return Err(error);
                }
            }
        }
        let result = match &mut binding.owner {
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::UsbProtocolControl => self
                .usb_protocol
                .as_mut()
                .ok_or(Refusal::InvalidPlan)?
                .control_invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::UsbProtocol),
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::UsbProtocolEndpoint
            | Owner::UsbDuplexRead
            | Owner::UsbDuplexWrite
            | Owner::UsbIsochronousOut => {
                return Err(Refusal::InvalidPlan);
            }
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::UsbDuplexControl => {
                let host = self.usb_duplex.as_mut().ok_or(Refusal::InvalidPlan)?;
                // SAFETY: can_dispatch above independently checked original
                // bulk Kernel obligations and dispatcher tokens before admission.
                match unsafe { host.prepare_endpoint_recovery() } {
                    Ok(()) => host
                        .control_invoke(node, call.call, call.request, input)
                        .map(Some)
                        .map_err(Refusal::UsbProtocol),
                    Err(error) => Err(Refusal::UsbProtocol(error)),
                }
            }
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::UsbIsochronousControl => self
                .usb_isochronous
                .as_mut()
                .ok_or(Refusal::InvalidPlan)?
                .control_invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::UsbIsochronous),
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::UsbControl(owner) => owner
                .invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::Usb),
            #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
            Owner::EthernetFrame(owner) => owner
                .invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::EthernetFrame),
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-board"))
            ))]
            Owner::StorageAuthorize | Owner::StorageSettle => return Err(Refusal::InvalidPlan),
            Owner::Pure(owner) => owner.invoke(node, call.call, call.request, input),
            Owner::Spi(owner) => owner
                .invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::Spi),
            Owner::Digital(owner) => owner
                .invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::Digital),
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

    pub(super) fn complete_failure(
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
}
