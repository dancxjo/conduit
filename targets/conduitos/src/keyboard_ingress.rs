//! Fixed ordered keyboard ingress at the admitted portable value boundary.
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

#[cfg(test)]
mod tests {
    use super::*;
    fn transition(usage: u8, pressed: bool) -> HidKeyTransition {
        HidKeyTransition::new(usage, pressed, 0)
    }
    #[test]
    fn ingress_preserves_all_admitted_transition_order_across_work_slices() {
        let mut ingress = KeyboardIngress::new();
        for usage in [4, 5, 6, 7] {
            ingress.admit(transition(usage, true)).unwrap();
        }

        let mut observed = [0_u8; 4];
        let mut count = 0;
        assert_eq!(
            ingress.service(2, |event| {
                observed[count] = event.usage();
                count += 1;
            }),
            2
        );
        assert_eq!(ingress.pending(), 2);
        assert_eq!(
            ingress.service(2, |event| {
                observed[count] = event.usage();
                count += 1;
            }),
            2
        );
        assert_eq!(ingress.pending(), 0);
        assert_eq!(observed, [4, 5, 6, 7]);
    }

    #[test]
    fn ingress_refuses_pressure_without_dropping_admitted_events() {
        let mut ingress = KeyboardIngress::new();
        for usage in 4..(4 + INGRESS_CAPACITY as u8) {
            ingress.admit(transition(usage, true)).unwrap();
        }
        assert_eq!(ingress.pending(), INGRESS_CAPACITY);
        assert_eq!(
            ingress.admit(transition(42, true)),
            Err(KeyboardIngressRefusal::Pressure)
        );

        let mut observed = [0_u8; INGRESS_CAPACITY];
        let mut count = 0;
        ingress.service(INGRESS_CAPACITY, |event| {
            observed[count] = event.usage();
            count += 1;
        });
        assert_eq!(count, INGRESS_CAPACITY);
        assert_eq!(observed[0], 4);
        assert_eq!(
            observed[INGRESS_CAPACITY - 1],
            4 + INGRESS_CAPACITY as u8 - 1
        );
    }

    #[test]
    fn report_pressure_refuses_before_any_transition_from_that_report_is_admitted() {
        let mut ingress = KeyboardIngress::new();
        for usage in 4..(4 + INGRESS_CAPACITY as u8 - 1) {
            ingress.admit(transition(usage, true)).unwrap();
        }
        let report = [transition(40, true), transition(40, false)];
        assert_eq!(
            ingress.admit_report(&report),
            Err(KeyboardIngressRefusal::Pressure)
        );
        assert_eq!(ingress.pending(), INGRESS_CAPACITY - 1);
        let mut observed = [0_u8; INGRESS_CAPACITY - 1];
        let mut count = 0;
        ingress.service(INGRESS_CAPACITY, |event| {
            observed[count] = event.usage();
            count += 1;
        });
        assert_eq!(count, INGRESS_CAPACITY - 1);
        assert!(!observed.contains(&40));
    }

    #[test]
    fn malformed_report_suffix_preserves_the_preexisting_queue() {
        let mut ingress = KeyboardIngress::new();
        ingress.admit(transition(4, true)).unwrap();
        assert_eq!(
            ingress.admit_report(&[transition(5, true), transition(0, true)]),
            Err(KeyboardIngressRefusal::InvalidTransition)
        );
        assert_eq!(ingress.pending(), 1);
        assert_eq!(ingress.service(8, |event| assert_eq!(event.usage(), 4)), 1);
    }
}
