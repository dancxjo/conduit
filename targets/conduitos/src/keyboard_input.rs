//! Consumers of validated HID transitions for proof and ordinary guest profiles.

use conduit_human::KeyEvent;
use conduit_semantic_catalog::KEYBOARD_MAX_QUEUE_ITEMS;

use crate::{
    arch::{HidKeyTransition, HidKeyboardSession, UsbDevice, XhciReady},
    keyboard_bridge,
    keyboard_plan::PreparedKeyboardPlay,
    keyboard_play,
};

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
/// HID/PS2 adapters put validated physical transitions here before any Form,
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

    /// Run at most `budget` admitted semantic deliveries. Presentation is
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortablePairConsumer {
    pending: [Option<KeyEvent>; 2],
    count: usize,
}

impl PortablePairConsumer {
    pub const fn new() -> Self {
        Self {
            pending: [None; 2],
            count: 0,
        }
    }

    pub fn accept(
        &mut self,
        transition: HidKeyTransition,
    ) -> Result<Option<[KeyEvent; 2]>, &'static str> {
        let value = keyboard_bridge::portable_key_event(
            transition.usage(),
            transition.pressed(),
            transition.modifiers(),
        )
        .map_err(|_| "keyboard-portable-value-invalid")?;
        self.pending[self.count] = Some(value);
        self.count += 1;
        if self.count != 2 {
            return Ok(None);
        }
        let pair = [self.pending[0].unwrap(), self.pending[1].unwrap()];
        self.pending = [None; 2];
        self.count = 0;
        Ok(Some(pair))
    }
}

impl Default for PortablePairConsumer {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs the ordinary guest keyboard source with fixed two-value Plays.
///
/// Session/report storage is reused on every iteration; no proof transcript is
/// retained and no particular usage or text sequence is expected.
pub fn run_interactive(
    session: &mut HidKeyboardSession,
    controller: &mut XhciReady,
    device: &UsbDevice,
    prepared: &PreparedKeyboardPlay,
    mut observe: impl FnMut(HidKeyTransition),
    mut interact: impl FnMut(HidKeyTransition) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    let mut consumer = PortablePairConsumer::new();
    for transition in session.transitions().iter().copied() {
        consume(prepared, &mut consumer, transition, &mut observe)?;
        interact(transition)?;
    }
    loop {
        let (transitions, count) = session
            .receive_followup(controller, device)
            .map_err(|error| error.as_str())?;
        for transition in transitions[..count].iter().copied() {
            consume(prepared, &mut consumer, transition, &mut observe)?;
            interact(transition)?;
        }
    }
}

/// Runs the ordinary physical input source for a long-lived product-owned
/// Play. Physical transitions cross the local-rescue boundary while their
/// provenance is still known, then are canonicalized into the exact finite
/// `input/keyboard` queue. The next USB transfer is armed before a bounded
/// semantic/presentation service slice drains portable `KeyEvent` values.
pub fn run_product(
    session: &mut HidKeyboardSession,
    controller: &mut XhciReady,
    device: &UsbDevice,
    mut interact: impl FnMut(ProductInputEvent) -> Result<ProductInputControl, &'static str>,
) -> Result<(), &'static str> {
    let mut ingress = KeyboardIngress::new();
    for transition in session.transitions().iter().copied() {
        if interact(ProductInputEvent::LocalRescue(
            transition.into_local_rescue(),
        ))? == ProductInputControl::Yield
        {
            return Ok(());
        }
    }
    ingress
        .admit_report(session.transitions())
        .map_err(KeyboardIngressRefusal::as_str)?;

    loop {
        // Publish the next transfer before giving admitted semantic work or
        // presentation/export a turn.
        session
            .begin_followup(controller, device)
            .map_err(|error| error.as_str())?;
        if service_product_ingress(&mut ingress, &mut interact)? == ProductInputControl::Yield {
            return Ok(());
        }

        let (transitions, count) = loop {
            match session.poll_followup(controller, device) {
                Ok(Some(batch)) => break batch,
                Ok(None) => {
                    if service_product_ingress(&mut ingress, &mut interact)?
                        == ProductInputControl::Yield
                    {
                        return Ok(());
                    }
                    core::hint::spin_loop();
                }
                Err(error) => {
                    let reason = error.as_str();
                    interact(ProductInputEvent::Lost(reason))?;
                    return Err(reason);
                }
            }
        };

        for transition in transitions[..count].iter().copied() {
            if interact(ProductInputEvent::LocalRescue(
                transition.into_local_rescue(),
            ))? == ProductInputControl::Yield
            {
                return Ok(());
            }
        }
        // Whole-report admission is atomic. We intentionally do not run Form
        // work here: the loop immediately publishes the next receive transfer,
        // then services these portable events within the admitted bound.
        ingress
            .admit_report(&transitions[..count])
            .map_err(KeyboardIngressRefusal::as_str)?;
    }
}

/// Runs the same long-lived portable keyboard source from a validated PS/2
/// mechanism. The adapter polls only a finite controller-byte window before a
/// bounded semantic service slice, retaining decoder prefixes between turns.
pub fn run_ps2_product(
    input: &mut crate::arch::Ps2Input,
    mut interact: impl FnMut(ProductInputEvent) -> Result<ProductInputControl, &'static str>,
) -> Result<(), &'static str> {
    let mut ingress = KeyboardIngress::new();
    loop {
        for _ in 0..INGRESS_CAPACITY {
            match input.poll_keyboard() {
                Ok(Some(transition)) => {
                    if interact(ProductInputEvent::LocalRescue(
                        transition.into_local_rescue(),
                    ))? == ProductInputControl::Yield
                    {
                        return Ok(());
                    }
                    ingress
                        .admit(transition)
                        .map_err(KeyboardIngressRefusal::as_str)?;
                }
                Ok(None) => break,
                Err(error) => {
                    let reason = error.as_str();
                    interact(ProductInputEvent::Lost(reason))?;
                    return Err(reason);
                }
            }
        }
        if service_product_ingress(&mut ingress, &mut interact)? == ProductInputControl::Yield {
            return Ok(());
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductInputControl {
    Continue,
    Yield,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductInputEvent {
    /// Host service opportunity, distinct from a physical input transition.
    Service,
    /// Proven-local physical transition for the narrow rescue policy only.
    LocalRescue(crate::local_rescue::ValidatedLocalTransition),
    /// Portable semantic keyboard occurrence drained from admitted ingress.
    Key(KeyEvent),
    Lost(&'static str),
}

/// Deliver at most the exact admitted keyboard queue capacity, then give the
/// host one separate presentation/export service opportunity. Portable
/// `KeyEvent` is the stored and delivered semantic truth; physical provenance
/// never needs to be reconstructed after this boundary.
fn service_product_ingress(
    ingress: &mut KeyboardIngress,
    interact: &mut impl FnMut(ProductInputEvent) -> Result<ProductInputControl, &'static str>,
) -> Result<ProductInputControl, &'static str> {
    for _ in 0..INGRESS_CAPACITY {
        let Some(event) = ingress.take() else {
            break;
        };
        if interact(ProductInputEvent::Key(event))? == ProductInputControl::Yield {
            return Ok(ProductInputControl::Yield);
        }
    }
    interact(ProductInputEvent::Service)
}

fn consume(
    prepared: &PreparedKeyboardPlay,
    consumer: &mut PortablePairConsumer,
    transition: HidKeyTransition,
    observe: &mut impl FnMut(HidKeyTransition),
) -> Result<(), &'static str> {
    observe(transition);
    if let Some(values) = consumer.accept(transition)? {
        keyboard_play::run(prepared, values).map_err(|_| "keyboard-play-refused")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transition(usage: u8, pressed: bool) -> HidKeyTransition {
        HidKeyTransition::new(usage, pressed, 0)
    }

    #[test]
    fn same_validated_producer_feeds_scripted_and_repeated_ordinary_consumers() {
        let scripted = [transition(4, true), transition(4, false)];
        assert_eq!(scripted[0].usage(), 4);

        let mut ordinary = PortablePairConsumer::new();
        for usage in [58, 5, 30] {
            assert!(ordinary.accept(transition(usage, true)).unwrap().is_none());
            let pair = ordinary.accept(transition(usage, false)).unwrap().unwrap();
            assert_eq!(pair[0].usage(), usage);
            assert_eq!(pair[1].usage(), usage);
        }
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

    #[test]
    fn product_service_delivers_portable_queue_in_order_before_service_phase() {
        let mut ingress = KeyboardIngress::new();
        ingress.admit(transition(4, true)).unwrap();
        ingress.admit(transition(5, true)).unwrap();

        let mut usages = [0_u8; 2];
        let mut count = 0;
        let mut service_seen = false;
        let control = service_product_ingress(&mut ingress, &mut |event| match event {
            ProductInputEvent::Key(event) => {
                usages[count] = event.usage();
                count += 1;
                Ok(ProductInputControl::Continue)
            }
            ProductInputEvent::Service => {
                service_seen = true;
                Ok(ProductInputControl::Continue)
            }
            ProductInputEvent::LocalRescue(_) => panic!("unexpected local-rescue observation"),
            ProductInputEvent::Lost(_) => panic!("unexpected loss"),
        })
        .unwrap();

        assert_eq!(control, ProductInputControl::Continue);
        assert_eq!(usages, [4, 5]);
        assert_eq!(count, 2);
        assert!(service_seen);
        assert_eq!(ingress.pending(), 0);
    }
}
