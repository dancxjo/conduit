//! Availability and bounded polling of retained native call obligations.
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-board"))
))]
use super::PendingUsbKind;
use super::{I2cProvider, Owner, PreparedProtocolCalls, ProtocolCallRefusal};
use conduit_composite::{KernelCompositeHost, KernelCompositeHostRequest};

impl<P: I2cProvider> PreparedProtocolCalls<P> {
    pub fn can_dispatch(
        &self,
        kernel: &KernelCompositeHost,
        request: &KernelCompositeHostRequest,
    ) -> Result<bool, ProtocolCallRefusal> {
        let node = kernel
            .host_request_view(request)
            .map_err(ProtocolCallRefusal::Kernel)?
            .request
            .node;
        let binding = self
            .bindings
            .iter()
            .find(|binding| binding.node == node)
            .ok_or(ProtocolCallRefusal::Unsupported)?;
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-board"))
        ))]
        {
            if self.pending_usb.is_some() || self.pending_usb_failed {
                return Ok(false);
            }
            if matches!(
                binding.owner,
                Owner::UsbDuplexControl | Owner::UsbDuplexRead | Owner::UsbDuplexWrite
            ) && self
                .storage
                .as_ref()
                .is_some_and(|storage| !storage.allows_usb(node))
            {
                return Ok(false);
            }
            // Native idle and an empty dispatcher map do not retire the
            // original Kernel obligation. In particular, a recovery control
            // must not cross an unacknowledged bulk result or retained token.
            if matches!(binding.owner, Owner::UsbDuplexControl) {
                let host = self
                    .usb_duplex
                    .as_ref()
                    .ok_or(ProtocolCallRefusal::InvalidPlan)?;
                if kernel
                    .has_retained_host_call_for_nodes(
                        &self.identity.host_id,
                        &host.bulk_subordinate_nodes(),
                    )
                    .map_err(ProtocolCallRefusal::Kernel)?
                {
                    return Ok(false);
                }
            }
            let available = match binding.owner {
                Owner::UsbDuplexRead => self.pending_duplex[0].is_none(),
                Owner::UsbDuplexWrite => self.pending_duplex[1].is_none(),
                Owner::UsbDuplexControl
                | Owner::UsbControl(_)
                | Owner::UsbProtocolControl
                | Owner::UsbProtocolEndpoint
                | Owner::UsbIsochronousControl
                | Owner::UsbIsochronousOut => self.pending_duplex.iter().all(Option::is_none),
                _ => true,
            };
            Ok(available)
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-board"))
        )))]
        {
            let _ = binding;
            Ok(true)
        }
    }

    /// Poll at most one native event for the original admitted endpoint token.
    pub fn poll_pending(
        &mut self,
        kernel: &mut KernelCompositeHost,
    ) -> Result<bool, ProtocolCallRefusal> {
        if kernel.definition().internal_plan.plan_id != self.plan_id {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-board"))
        ))]
        if self.pending_duplex.iter().any(Option::is_some) {
            if self.pending_usb_failed {
                return Err(ProtocolCallRefusal::Unsupported);
            }
            let result = self
                .usb_duplex
                .as_mut()
                .ok_or(ProtocolCallRefusal::InvalidPlan)?
                .poll()
                .map_err(ProtocolCallRefusal::UsbProtocol);
            match result {
                Ok(Some((node, bytes))) => {
                    let direction = self
                        .pending_duplex
                        .iter()
                        .position(|pending| {
                            pending
                                .as_ref()
                                .is_some_and(|(expected, _)| *expected == node)
                        })
                        .ok_or(ProtocolCallRefusal::InvalidPlan)?;
                    let (_, admitted) =
                        self.pending_duplex[direction].ok_or(ProtocolCallRefusal::InvalidPlan)?;
                    kernel
                        .complete_host_call_bytes(&admitted, bytes)
                        .map_err(ProtocolCallRefusal::Kernel)?;
                    self.pending_duplex[direction] = None;
                }
                Ok(None) => {}
                Err(error) => {
                    self.pending_usb_failed = true;
                    for (_, admitted) in self.pending_duplex.iter().flatten() {
                        Self::complete_failure(kernel, admitted, &error)?;
                    }
                    return Err(error);
                }
            }
            // A pending IN must not block ordinary pure/queue calls or one OUT.
            return Ok(false);
        }
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-board"))
        ))]
        if let Some(admitted) = self.pending_usb {
            if self.pending_usb_failed {
                return Err(ProtocolCallRefusal::Unsupported);
            }
            let result = match self.pending_usb_kind {
                PendingUsbKind::ProtocolRead => self
                    .usb_protocol
                    .as_mut()
                    .ok_or(ProtocolCallRefusal::InvalidPlan)?
                    .endpoint_poll()
                    .map_err(ProtocolCallRefusal::UsbProtocol),
                PendingUsbKind::DuplexRead => self
                    .usb_duplex
                    .as_mut()
                    .ok_or(ProtocolCallRefusal::InvalidPlan)?
                    .read_poll()
                    .map_err(ProtocolCallRefusal::UsbProtocol),
                PendingUsbKind::DuplexWrite => self
                    .usb_duplex
                    .as_mut()
                    .ok_or(ProtocolCallRefusal::InvalidPlan)?
                    .write_poll()
                    .map_err(ProtocolCallRefusal::UsbProtocol),
                PendingUsbKind::IsochronousOut => self
                    .usb_isochronous
                    .as_mut()
                    .ok_or(ProtocolCallRefusal::InvalidPlan)?
                    .isochronous_poll()
                    .map_err(ProtocolCallRefusal::UsbIsochronous),
            };
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
