//! Fixed portable ingress shared by native keyboard providers.
use crate::{hid_key_transition::HidKeyTransition, keyboard_bridge};
use conduit_human::KeyEvent;
use conduit_semantic_catalog::KEYBOARD_MAX_QUEUE_ITEMS;

/// The exact source-queue capacity admitted by `input/keyboard`.
///
/// This is deliberately shared with the semantic contract: a native input
/// adapter may not hide a larger backlog below the planned keyboard Cord.
pub const INGRESS_CAPACITY: usize = KEYBOARD_MAX_QUEUE_ITEMS as usize;

/// A refusal while accepting a physical transition into the admitted source
/// queue. Pressure is distinct from malformed physical input, and neither
/// loses an already-admitted transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardIngressRefusal {
    InvalidTransition,
    Pressure,
}

impl KeyboardIngressRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidTransition => "keyboard-portable-value-invalid",
            Self::Pressure => "keyboard-input-pressure",
        }
    }
}

/// Fixed, ordered, portable keyboard ingress.
///
/// HID/PS2 adapters put validated physical transitions here before any Plot,
/// compositor, or Sign-export work runs. A host event loop can then take a
/// bounded work slice with [`Self::service`], while continuing to re-arm and
/// poll the physical mechanism independently.
pub struct KeyboardIngress {
    queue: crate::machine::FixedFactRing<KeyEvent, INGRESS_CAPACITY>,
}

impl KeyboardIngress {
    pub const fn new() -> Self {
        Self {
            queue: crate::machine::FixedFactRing::new(),
        }
    }

    /// Canonicalize and admit one validated physical transition. The queue
    /// remains ordered; transitions are never coalesced.
    pub fn admit(&mut self, transition: HidKeyTransition) -> Result<(), KeyboardIngressRefusal> {
        let event = keyboard_bridge::portable_key_event(
            transition.usage(),
            transition.pressed(),
            transition.modifiers(),
        )
        .map_err(|_| KeyboardIngressRefusal::InvalidTransition)?;
        self.queue
            .push(event)
            .map_err(|_| KeyboardIngressRefusal::Pressure)
    }

    /// Admit all transitions derived from one validated physical report, or
    /// refuse before changing the queue when its finite capacity cannot hold
    /// the complete ordered report.
    pub fn admit_report(
        &mut self,
        transitions: &[HidKeyTransition],
    ) -> Result<(), KeyboardIngressRefusal> {
        if transitions.len() > INGRESS_CAPACITY.saturating_sub(self.pending()) {
            return Err(KeyboardIngressRefusal::Pressure);
        }
        // Validate the whole report before mutating the queue as well: a
        // malformed final transition must not leave a successfully queued prefix.
        for transition in transitions {
            keyboard_bridge::portable_key_event(
                transition.usage(),
                transition.pressed(),
                transition.modifiers(),
            )
            .map_err(|_| KeyboardIngressRefusal::InvalidTransition)?;
        }
        for transition in transitions {
            self.admit(*transition)?;
        }
        Ok(())
    }

    /// Run at most `budget` admitted semantic deliveries. Face is
    /// intentionally not part of this operation; consumers return after the
    /// semantic input boundary and let a separate dirty-driven frame phase
    /// decide whether to compose.
    pub fn service(&mut self, budget: usize, mut deliver: impl FnMut(KeyEvent)) -> usize {
        let mut delivered = 0;
        while delivered < budget {
            let Some(event) = self.queue.pop() else {
                break;
            };
            deliver(event);
            delivered += 1;
        }
        delivered
    }

    /// Take one already-admitted portable value for a fallible product service
    /// phase. Keeping this separate from [`Self::service`] lets callers stop on
    /// an exact Yield/refusal without accidentally consuming later events.
    pub fn take(&mut self) -> Option<KeyEvent> {
        self.queue.pop()
    }

    pub const fn pending(&self) -> usize {
        self.queue.len()
    }
}

impl Default for KeyboardIngress {
    fn default() -> Self {
        Self::new()
    }
}
