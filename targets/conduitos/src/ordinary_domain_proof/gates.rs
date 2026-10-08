//! Hostile-image fixtures exercise the production gate, separately from Source evidence.
use super::refuse;
use crate::{
    arch,
    composition::MachineRunError as Error,
    machine::{BaseError, SerialBase},
    offer::HostOffer,
    protected_region::{DomainFault, DomainRefusal, DomainState},
    protection_domain::{KernelCapabilityRefusal as Capability, KernelRevocationCause as Cause},
    text_protection::ProtectedText,
};
use conduit_core::Plan;

pub(super) fn fixture(plan: &Plan, offer: &HostOffer<'_>) -> ProtectedText {
    let fragment = &plan.fragments[0];
    let active =
        crate::ordinary_plan::new_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id)
            .unwrap_or_else(|_| refuse("gate-fixture-play"));
    let mut domain = ProtectedText::prepare(plan, &active, offer)
        .unwrap_or_else(|_| refuse("gate-fixture-prepare"));
    domain.mark_fixture();
    domain
}

pub(super) fn run(plan: &Plan, offer: &HostOffer<'_>) {
    for (probe, expected, effects) in [
        (8, Error::ProtectionCapability(Capability::UnknownHandle), 0),
        (10, Error::ProtectionCapability(Capability::WrongScope), 0),
        (11, Error::ProtectionDomain(DomainRefusal::InvalidMemory), 0),
        (12, Error::ProtectionCapability(Capability::WorkEnvelope), 0),
        (13, Error::ProtectionDomain(DomainRefusal::InvalidMemory), 0),
        (14, Error::ProtectionFault(DomainFault::InvalidGate), 1),
        (15, Error::ProtectionFault(DomainFault::InvalidGate), 0),
        (16, Error::ProtectionFault(DomainFault::InvalidGate), 0),
    ] {
        let mut domain = fixture(plan, offer);
        domain.gate_probe(probe, 0);
        let mut serial = arch::Serial::new();
        if domain.present(b"gate-negative", &mut serial) != Err(expected)
            || serial.presentation_count() != effects
            || domain.state_for_probe() != DomainState::Faulted(DomainFault::InvalidGate)
            || domain.present(b"replay", &mut serial).is_ok()
            || serial.presentation_count() != effects
        {
            refuse("gate-negative-mismatch");
        }
    }
    {
        let sibling = fixture(plan, offer);
        let mut domain = fixture(plan, offer);
        domain.gate_probe(9, sibling.handle_for_probe());
        let mut serial = arch::Serial::new();
        if domain.present(b"stolen", &mut serial)
            != Err(Error::ProtectionCapability(Capability::UnknownHandle))
            || serial.presentation_count() != 0
        {
            refuse("sibling-handle-admitted");
        }
    }
    {
        let mut domain = fixture(plan, offer);
        let mut serial = arch::Serial::new();
        if domain.present(b"once", &mut serial).is_err()
            || domain.present(b"twice", &mut serial)
                != Err(Error::ProtectionCapability(Capability::Exhausted))
            || serial.presentation_count() != 1
        {
            refuse("operation-count-not-enforced");
        }
    }
    for cause in [
        Cause::PlayCancelled,
        Cause::PlayCompleted,
        Cause::PlanReplaced,
        Cause::AuthorityRevoked,
        Cause::BaseReplaced,
        Cause::ResourceReplaced,
        Cause::BootReplaced,
        Cause::ProviderLost,
        Cause::ProtectionFault,
        Cause::PlayFailed,
    ] {
        let mut domain = fixture(plan, offer);
        domain.revoke(cause);
        let mut serial = arch::Serial::new();
        if domain.present(b"revoked", &mut serial).is_ok()
            || serial.presentation_count() != 0
            || domain.state_for_probe() != DomainState::Revoked(cause)
        {
            refuse("revoked-domain-operated-base");
        }
    }
    {
        let mut domain = fixture(plan, offer);
        let mut serial = LostProvider;
        if domain.present(b"lost", &mut serial) != Err(Error::SerialBaseFailure)
            || domain.state_for_probe() != DomainState::Revoked(Cause::ProviderLost)
        {
            refuse("provider-loss-not-revoked");
        }
    }
    for replace_before in [true, false] {
        let mut domain = fixture(plan, offer);
        let mut serial = ReplacedProvider {
            physical: arch::Serial::new(),
            generation: if replace_before { 2 } else { 1 },
        };
        if domain.present(b"replacement-fixture", &mut serial)
            != Err(Error::ProtectionDomain(DomainRefusal::WrongBinding))
            || serial.presentation_count() != u32::from(!replace_before)
            || domain.state_for_probe() != DomainState::Revoked(Cause::BaseReplaced)
            || domain.present(b"stale", &mut serial).is_ok()
            || serial.presentation_count() != u32::from(!replace_before)
        {
            refuse("provider-replacement-not-fenced");
        }
    }
    arch::early_write(b"CONDUIT_DOMAIN_GATE_NEGATIVES unknown-handle sibling-handle wrong-operation oversized-window excessive-work invalid-capacity invalid-utf8 forged-fault replay exhausted-operations revoked-lifecycle provider-loss provider-replacement\n");
}

struct LostProvider;
impl SerialBase for LostProvider {
    fn present(&mut self, _: &[u8]) -> Result<(), BaseError> {
        Err(BaseError::Unavailable)
    }
    fn presentation_count(&self) -> u32 {
        0
    }
    fn provider_generation(&self) -> Option<u64> {
        Some(1)
    }
}

// A Root fixture advances the offered epoch around a real serial operation.
// This checks the gate's race fence; it is not evidence of hardware hotplug.
struct ReplacedProvider {
    physical: arch::Serial,
    generation: u64,
}
impl SerialBase for ReplacedProvider {
    fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
        self.physical.present(bytes)?;
        self.generation = 2;
        Ok(())
    }
    fn presentation_count(&self) -> u32 {
        self.physical.presentation_count()
    }
    fn provider_generation(&self) -> Option<u64> {
        Some(self.generation)
    }
}
