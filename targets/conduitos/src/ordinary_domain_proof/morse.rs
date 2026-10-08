//! Same checked Tour Source through the normal protected Play on each backend.
use super::refuse;
use crate::{
    arch,
    machine::{BaseError, SerialBase},
    offer::HostOffer,
};
use conduit_core::Plan;

pub(super) fn run(_: &Plan, offer: &HostOffer<'_>) {
    let ids = crate::identity::BootIdentities {
        host: offer.host_id,
        boot: offer.boot_id,
    };
    let mut prepared =
        crate::tour_morse_plan::prepare(&ids, offer, crate::make::EMBEDDED_MAKE.build_id)
            .unwrap_or_else(|_| refuse("morse-product-prepare"));
    let expected = conduit_text::MorsePattern::from_text("SOS", 80)
        .unwrap_or_else(|_| refuse("morse-proof-expected"))
        .encode()
        .unwrap_or_else(|_| refuse("morse-proof-encoding"));
    let mut serial = ObservedSerial {
        inner: arch::Serial::new(),
        expected: &expected,
        text: false,
        pattern: false,
        count: 0,
    };
    let receipt = crate::tour_morse_play::run(
        &mut prepared,
        &mut arch::Clock::new(),
        &mut serial,
        &mut arch::Interrupts::new(),
        &mut arch::Idle::new(),
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    let (state, cost) = prepared.protected.proof_state();
    if !serial.text
        || !serial.pattern
        || receipt.serial_presentations != 2
        || receipt.pending_host_calls != 0
        || cost.entries != 5
        || cost.base_gate_transitions != 2
        || cost.teardown_zeroed_bytes != cost.reserved_bytes
        || state
            != crate::protected_region::DomainState::Revoked(
                crate::protection_domain::KernelRevocationCause::PlayCompleted,
            )
    {
        refuse("morse-product-output-cost-or-lifecycle");
    }
    use core::fmt::Write;
    let mut sign = crate::sign_format::FixedText::new();
    writeln!(sign, "CONDUIT_DOMAIN_MORSE {{\"schema\":\"conduit.conduitos/protected-tour-morse@1\",\"proof_class\":\"freestanding-emulator\",\"architecture\":\"{}\",\"plan_id\":\"{}\",\"play_id\":\"{}\",\"source_document_id\":\"{}\",\"checked_plot_id\":\"{}\",\"expanded_plot_id\":\"{}\",\"canonical_pattern\":true,\"serial_effects\":2,\"dma_isolation\":false,\"driver_isolation\":false}}",
        arch::ARCHITECTURE, prepared.plan_id.as_str(), prepared.active_play.active_play_id.as_str(),
        prepared.source_document_id.as_str(), prepared.checked_plot_id.as_str(),
        prepared.expanded_plot_id.as_str()).unwrap_or_else(|_| refuse("morse-proof-sign-capacity"));
    arch::early_write(b"\n");
    drop(prepared);
    arch::early_write(sign.as_bytes());
    independent_handles(offer);
    independent_branches(offer, &expected);
}

// Observe the existing serial provider. This proves a serial diagnostic effect,
// not a physical indicator device or DMA/driver isolation.
struct ObservedSerial<'a> {
    inner: arch::Serial,
    expected: &'a [u8],
    text: bool,
    pattern: bool,
    count: u32,
}
impl SerialBase for ObservedSerial<'_> {
    fn present(&mut self, value: &[u8]) -> Result<(), BaseError> {
        if value == b"SOS" && !self.text {
            self.text = true;
        } else if value == self.expected && !self.pattern {
            self.pattern = true;
        } else {
            refuse("morse-product-unexpected-effect");
        }
        self.inner.present(value)?;
        self.count += 1;
        Ok(())
    }
    fn presentation_count(&self) -> u32 {
        self.count
    }
    fn provider_generation(&self) -> Option<u64> {
        self.inner.provider_generation()
    }
}

fn independent_handles(offer: &HostOffer<'_>) {
    use crate::{
        composition::MachineRunError as Error,
        protected_region::{DomainFault, DomainState},
        protection_domain::KernelCapabilityRefusal as Capability,
    };
    let ids = crate::identity::BootIdentities {
        host: offer.host_id,
        boot: offer.boot_id,
    };
    for text in [false, true] {
        let mut prepared =
            crate::tour_morse_plan::prepare(&ids, offer, crate::make::EMBEDDED_MAKE.build_id)
                .unwrap_or_else(|_| refuse("morse-handle-fixture-prepare"));
        let domain = &mut prepared.protected;
        domain.proof_fixture(true);
        domain
            .uppercase(b"sos")
            .unwrap_or_else(|_| refuse("morse-handle-fixture-compute"));
        let mut pattern = [0; conduit_text::MAXIMUM_MORSE_PATTERN_BYTES];
        let value = domain
            .morse(b"sos")
            .unwrap_or_else(|_| refuse("morse-handle-fixture-cord"));
        let length = value.len();
        pattern[..length].copy_from_slice(value);
        let value = if text {
            &b"SOS"[..]
        } else {
            &pattern[..length]
        };
        let mut serial = arch::Serial::new();
        if domain.present(value, text, &mut serial)
            != Err(Error::ProtectionCapability(Capability::WrongScope))
            || serial.presentation_count() != 0
            || domain.proof_state().0 != DomainState::Faulted(DomainFault::InvalidGate)
            || domain.present(value, text, &mut serial).is_ok()
            || serial.presentation_count() != 0
        {
            refuse("morse-effect-handle-cross-use");
        }
    }
    arch::early_write(b"CONDUIT_DOMAIN_MORSE_HANDLES text-indicator-cross-use-refused fault-revoked no-effect replay-refused\n");
}

fn independent_branches(offer: &HostOffer<'_>, expected: &[u8]) {
    use crate::composition::MachineRunError as Error;
    let ids = crate::identity::BootIdentities {
        host: offer.host_id,
        boot: offer.boot_id,
    };
    for morse_first in [false, true] {
        let mut prepared =
            crate::tour_morse_plan::prepare(&ids, offer, crate::make::EMBEDDED_MAKE.build_id)
                .unwrap_or_else(|_| refuse("morse-order-fixture-prepare"));
        let domain = &mut prepared.protected;
        domain.proof_fixture(false);
        if morse_first {
            if domain.morse(b"sos").map(|value| value == expected) != Ok(true)
                || domain.uppercase(b"SOS") != Err(Error::KernelFailure)
                || domain.uppercase(b"sos").map(|value| value == b"SOS") != Ok(true)
            {
                refuse("morse-first-fanout");
            }
        } else if domain.uppercase(b"sos").map(|value| value == b"SOS") != Ok(true)
            || domain.morse(b"SOS") != Err(Error::KernelFailure)
            || domain.morse(b"sos").map(|value| value == expected) != Ok(true)
        {
            refuse("uppercase-first-fanout");
        }
        if domain.proof_state().1.entries != 1 || domain.morse(b"SOS").is_ok() {
            refuse("morse-fanout-entry-or-cord");
        }
    }
    let mut prepared =
        crate::tour_morse_plan::prepare(&ids, offer, crate::make::EMBEDDED_MAKE.build_id)
            .unwrap_or_else(|_| refuse("morse-refusal-fixture-prepare"));
    let domain = &mut prepared.protected;
    domain.proof_fixture(false);
    if domain.uppercase("ß".as_bytes()).map(|value| value == b"SS") != Ok(true)
        || domain.morse("ß".as_bytes()) != Err(Error::KernelFailure)
        || domain.proof_state().1.entries != 1
    {
        refuse("morse-was-fed-uppercase-output");
    }
    drop(prepared);
    arch::early_write(b"CONDUIT_DOMAIN_MORSE_FANOUT original-input either-request-order independent-refusals changed-input-refused\n");
}
