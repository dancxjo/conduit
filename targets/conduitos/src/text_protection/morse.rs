//! One pure domain entry realizes the selected Upper and Morse implementations.
use super::*;
use crate::{domain_serial_scope::SerialScope, protection_domain::KernelCapabilityHandle};
use conduit_core::ConfigurationValue;

const PATTERN_BYTES: usize = conduit_text::MAXIMUM_MORSE_PATTERN_BYTES;

pub(crate) struct ProtectedMorse {
    domain: ProtectedText,
    indicator: SerialScope,
    indicator_handle: KernelCapabilityHandle,
    unit: u16,
    upper: UppercaseText,
    pattern: [u8; PATTERN_BYTES],
    pattern_length: usize,
    morse_status: u32,
    computed: bool,
    morse_served: bool,
}

impl ProtectedMorse {
    pub fn prepare(
        plan: &Plan,
        active: &ActivePlayIdentity,
        fixed: &crate::offer::HostOffer<'_>,
    ) -> Result<Self, MachineRunError> {
        let fragment = plan
            .fragments
            .first()
            .ok_or(MachineRunError::KernelConstruction)?;
        if plan.fragments.len() != 1 || fragment.placements.len() != 5 {
            return Err(MachineRunError::KernelConstruction);
        }
        // Match every admitted implementation artifact to the current offer,
        // including pure implementations that receive no Base handle.
        for placement in &fragment.placements {
            if !fixed.capabilities.iter().any(|capability| {
                capability.kind == placement.kind_id.as_str()
                    && capability.contract_revision == placement.kind_contract_revision.as_str()
                    && capability.implementation == placement.implementation_id.as_str()
                    && placement.artifact_id.as_str()
                        == alloc::format!("conduitos-build/{}", capability.artifact_build)
            }) {
                return Err(MachineRunError::KernelConstruction);
            }
        }
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == conduit_text::TEXT_MORSE_KIND)
            .ok_or(MachineRunError::KernelConstruction)?;
        let [configuration] = placement.configuration.as_slice() else {
            return Err(MachineRunError::KernelConstruction);
        };
        let unit = match (&configuration.value, configuration.key.as_str()) {
            (ConfigurationValue::U64(value), conduit_text::MORSE_UNIT_MILLIS_KEY) => {
                u16::try_from(*value).ok().filter(|unit| {
                    (conduit_text::MINIMUM_MORSE_UNIT_MILLIS
                        ..=conduit_text::MAXIMUM_MORSE_UNIT_MILLIS)
                        .contains(unit)
                })
            }
            _ => None,
        }
        .ok_or(MachineRunError::KernelConstruction)?;
        let mut domain = ProtectedText::prepare_with_kernel_bytes(
            plan,
            active,
            fixed,
            core::mem::size_of::<crate::tour_morse_kernel::TourMorseKernel>()
                + core::mem::size_of::<Self>(),
        )?;
        let indicator = SerialScope::admit_indicator(plan, &domain.current, fixed)
            .map_err(MachineRunError::ProtectionDomain)?;
        let indicator_handle = domain
            .capabilities
            .issue(domain.current.domain, indicator.scope)
            .map_err(MachineRunError::ProtectionCapability)?;
        Ok(Self {
            domain,
            indicator,
            indicator_handle,
            unit,
            upper: UppercaseText {
                bytes: [0; MAXIMUM_BYTES],
                len: 0,
            },
            pattern: [0; PATTERN_BYTES],
            pattern_length: 0,
            morse_status: 0,
            computed: false,
            morse_served: false,
        })
    }

    pub fn uppercase(&mut self, input: &[u8]) -> Result<&[u8], MachineRunError> {
        if self.computed {
            return Err(MachineRunError::KernelFailure);
        }
        self.domain
            .region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .morse_chain_input(input, self.unit)
            .map_err(MachineRunError::ProtectionDomain)?;
        self.domain.return_from_pure()?;
        let backend = self
            .domain
            .region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?;
        match backend.status() {
            0 => {}
            1 => return Err(MachineRunError::TextMalformedUtf8),
            2 => return Err(MachineRunError::TextOutputOverflow),
            _ => return Err(MachineRunError::KernelFailure),
        }
        self.upper.len = backend
            .output(&mut self.upper.bytes)
            .map_err(MachineRunError::ProtectionDomain)?;
        if core::str::from_utf8(self.upper.as_bytes()).is_err() {
            return Err(MachineRunError::ProtectionDomain(
                crate::protected_region::DomainRefusal::InvalidMemory,
            ));
        }
        (self.pattern_length, self.morse_status) = backend
            .morse_output(&mut self.pattern)
            .map_err(MachineRunError::ProtectionDomain)?;
        self.computed = true;
        Ok(self.upper.as_bytes())
    }

    pub fn morse(&mut self, input: &[u8]) -> Result<&[u8], MachineRunError> {
        if !self.computed
            || self.morse_served
            || input != self.upper.as_bytes()
            || self.morse_status != 0
        {
            return Err(MachineRunError::KernelFailure);
        }
        self.morse_served = true;
        Ok(&self.pattern[..self.pattern_length])
    }

    pub fn present(
        &mut self,
        input: &[u8],
        text: bool,
        serial: &mut impl crate::machine::SerialBase,
    ) -> Result<(), MachineRunError> {
        if !self.computed
            || (text && input != self.upper.as_bytes())
            || (!text && (!self.morse_served || input != &self.pattern[..self.pattern_length]))
        {
            return Err(MachineRunError::KernelFailure);
        }
        if text {
            self.domain.present(input, serial)
        } else {
            self.domain.present_selected(
                input,
                serial,
                self.indicator,
                self.indicator_handle,
                effect::Window::MORSE,
            )
        }
    }

    #[cfg(feature = "ordinary-domain-proof")]
    pub(crate) fn proof_fixture(&mut self, swap_handles: bool) {
        self.domain.mark_fixture();
        if swap_handles {
            core::mem::swap(&mut self.domain.serial_handle, &mut self.indicator_handle);
        }
    }

    #[cfg(feature = "ordinary-domain-proof")]
    pub(crate) fn proof_state(
        &self,
    ) -> (
        crate::protected_region::DomainState,
        crate::protected_region::DomainCost,
    ) {
        (self.domain.region.state(), self.domain.region.cost())
    }

    pub fn revoke(&mut self, cause: KernelRevocationCause) {
        self.domain.revoke(cause);
    }
}
