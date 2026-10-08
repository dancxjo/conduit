//! Exact capability-gated bounded presentation and completion.
use super::*;

impl<I: TextOwner> ProtectedText<I> {
    pub fn present(
        &mut self,
        input: &[u8],
        serial: &mut impl crate::machine::SerialBase,
    ) -> Result<(), MachineRunError> {
        self.present_selected(input, serial, self.serial, self.serial_handle, Window::TEXT)
    }

    pub(super) fn present_selected<const N: usize>(
        &mut self,
        input: &[u8],
        serial: &mut impl crate::machine::SerialBase,
        selected: crate::domain_serial_scope::SerialScope,
        handle: crate::protection_domain::KernelCapabilityHandle,
        window: Window<N>,
    ) -> Result<(), MachineRunError> {
        use crate::protected_region::DomainFault;
        let current = self
            .current
            .scope(&selected, serial.provider_generation())
            .map_err(|refusal| {
                self.revoke(KernelRevocationCause::BaseReplaced);
                MachineRunError::ProtectionDomain(refusal)
            })?;
        (window.prepare)(
            self.region
                .backend_mut()
                .map_err(MachineRunError::ProtectionDomain)?,
            input,
            handle.raw_for_domain(),
        )
        .map_err(MachineRunError::ProtectionDomain)?;
        match self
            .region
            .resume(&self.current, 1, &mut self.capabilities)
            .map_err(MachineRunError::ProtectionDomain)?
        {
            DomainReturn::Gate => {}
            DomainReturn::Fault(fault) => return Err(MachineRunError::ProtectionFault(fault)),
            _ => {
                self.region
                    .fault(DomainFault::InvalidGate, &mut self.capabilities);
                return Err(MachineRunError::ProtectionFault(DomainFault::InvalidGate));
            }
        }
        let mut copied = [0; N];
        let (raw, operation, work_units, length) = (window.request)(
            self.region
                .backend_mut()
                .map_err(MachineRunError::ProtectionDomain)?,
            &mut copied,
        )
        .map_err(|error| {
            self.region
                .fault(DomainFault::InvalidGate, &mut self.capabilities);
            MachineRunError::ProtectionDomain(error)
        })?;
        if length > N
            || work_units == 0
            || (window.utf8 && core::str::from_utf8(&copied[..length]).is_err())
        {
            self.region
                .fault(DomainFault::InvalidGate, &mut self.capabilities);
            return Err(MachineRunError::ProtectionFault(DomainFault::InvalidGate));
        }
        let claim = crate::protection_domain::KernelOperationClaim {
            boot: current.boot,
            plan: current.plan,
            play: current.play,
            base_generation: current.base_generation,
            resource_generation: current.resource_generation,
            operation,
            parameter_bytes: length as u32,
            work_units,
        };
        let lease = self
            .capabilities
            .authorize_current(
                self.current.domain(),
                crate::protection_domain::KernelCapabilityHandle::from_untrusted(raw),
                &current,
                claim,
            )
            .map_err(|error| {
                self.region
                    .fault(DomainFault::InvalidGate, &mut self.capabilities);
                MachineRunError::ProtectionCapability(error)
            })?;
        if serial.present(&copied[..length]).is_err() {
            self.revoke(KernelRevocationCause::ProviderLost);
            return Err(MachineRunError::SerialBaseFailure);
        }
        if self
            .current
            .scope(&selected, serial.provider_generation())
            .is_err()
        {
            self.revoke(KernelRevocationCause::BaseReplaced);
            return Err(MachineRunError::ProtectionDomain(
                crate::protected_region::DomainRefusal::WrongBinding,
            ));
        }
        self.capabilities
            .complete(lease)
            .map_err(MachineRunError::ProtectionCapability)?;
        self.region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .effect_completed();
        match self
            .region
            .resume(&self.current, 1, &mut self.capabilities)
            .map_err(MachineRunError::ProtectionDomain)?
        {
            DomainReturn::Yielded => {
                if self
                    .region
                    .backend_mut()
                    .map_err(MachineRunError::ProtectionDomain)?
                    .status()
                    != 0
                {
                    self.region
                        .fault(DomainFault::InvalidGate, &mut self.capabilities);
                    return Err(MachineRunError::ProtectionFault(DomainFault::InvalidGate));
                }
                Ok(())
            }
            DomainReturn::Fault(fault) => Err(MachineRunError::ProtectionFault(fault)),
            _ => {
                self.region
                    .fault(DomainFault::InvalidGate, &mut self.capabilities);
                Err(MachineRunError::ProtectionFault(DomainFault::InvalidGate))
            }
        }
    }
}

// These windows are chosen by Root from the selected effect, never by the domain.
type Prepare =
    fn(&mut TextDomain, &[u8], u64) -> Result<(), crate::protected_region::DomainRefusal>;
type Request<const N: usize> =
    fn(
        &mut TextDomain,
        &mut [u8; N],
    ) -> Result<(u64, u32, u32, usize), crate::protected_region::DomainRefusal>;
pub(super) struct Window<const N: usize> {
    prepare: Prepare,
    request: Request<N>,
    utf8: bool,
}
impl Window<MAXIMUM_BYTES> {
    const TEXT: Self = Self {
        prepare: TextDomain::presentation,
        request: TextDomain::effect_request,
        utf8: true,
    };
}

impl Window<{ conduit_text::MAXIMUM_MORSE_PATTERN_BYTES }> {
    pub(super) const MORSE: Self = Self {
        prepare: TextDomain::morse_presentation,
        request: TextDomain::morse_effect_request,
        utf8: false,
    };
}
