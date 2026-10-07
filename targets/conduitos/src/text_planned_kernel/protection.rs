//! Domain entry and gated effects beneath the existing production scheduler.
use super::*;

impl TextPlannedKernel {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    pub(crate) fn protect(
        &mut self,
        plan: &conduit_core::Plan,
        active: &conduit_core::ActivePlayIdentity,
        fixed: &crate::offer::HostOffer<'_>,
    ) -> Result<(), crate::composition::MachineRunError> {
        let next = crate::text_protection::ProtectedText::prepare(plan, active, fixed)?;
        if let Some(previous) = &mut self.protected {
            previous.revoke(crate::protection_domain::KernelRevocationCause::PlanReplaced);
        }
        self.protected = Some(next);
        Ok(())
    }

    pub(crate) fn compute_upper(
        &mut self,
        request: HostCallRequest,
    ) -> Result<crate::text_upper::UppercaseText, crate::composition::MachineRunError> {
        use crate::composition::MachineRunError as Error;
        if !self.is_upper_request(&request) {
            return Err(Error::UnexpectedHostCall);
        }
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|_| Error::KernelFailure)?;
        let result = {
            #[cfg(all(target_arch = "x86_64", target_os = "none"))]
            {
                self.protected
                    .as_mut()
                    .ok_or(Error::KernelConstruction)?
                    .uppercase(input)
            }
            #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
            {
                crate::text_upper::uppercase(input).map_err(|error| match error {
                    crate::text_upper::UppercaseError::MalformedUtf8 => Error::TextMalformedUtf8,
                    crate::text_upper::UppercaseError::OutputOverflow => Error::TextOutputOverflow,
                })
            }
        };
        if let Err(error) = result {
            #[cfg(all(target_arch = "x86_64", target_os = "none"))]
            if let Some(domain) = &mut self.protected {
                domain.revoke(crate::protection_domain::KernelRevocationCause::PlayFailed);
            }
            let (code, detail) = match error {
                Error::TextMalformedUtf8 => (conduit_kernel::FailureCode::InvalidInput, 1),
                Error::TextOutputOverflow => (conduit_kernel::FailureCode::StorageExhausted, 2),
                Error::ProtectionFault(crate::protected_region::DomainFault::WorkExhausted) => {
                    (conduit_kernel::FailureCode::WorkBudgetExhausted, 3)
                }
                _ => (conduit_kernel::FailureCode::HostCallFailed, 4),
            };
            self.scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Failed,
                        output: None,
                        failure: Some(conduit_kernel::Failure { code, detail }),
                    },
                )
                .map_err(|_| Error::KernelFailure)?;
        }
        result
    }

    pub(crate) fn present_value(
        &mut self,
        request: HostCallRequest,
        serial: &mut impl crate::machine::SerialBase,
    ) -> Result<(), crate::composition::MachineRunError> {
        use crate::composition::MachineRunError as Error;
        if !self.is_presentation_request(&request) {
            return Err(Error::UnexpectedHostCall);
        }
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|_| Error::KernelFailure)?;
        core::str::from_utf8(input).map_err(|_| Error::SerialBaseFailure)?;
        let result = {
            #[cfg(all(target_arch = "x86_64", target_os = "none"))]
            {
                self.protected
                    .as_mut()
                    .ok_or(Error::KernelConstruction)?
                    .present(input, serial)
            }
            #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
            {
                serial.present(input).map_err(|_| Error::SerialBaseFailure)
            }
        };
        if result.is_err() {
            #[cfg(all(target_arch = "x86_64", target_os = "none"))]
            if let Some(domain) = &mut self.protected {
                domain.revoke(crate::protection_domain::KernelRevocationCause::PlayFailed);
            }
            self.fail_presentation(request)
                .map_err(|_| Error::KernelFailure)?;
        }
        result
    }
}
