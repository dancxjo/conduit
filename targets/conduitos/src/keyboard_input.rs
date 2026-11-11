//! Consumers of validated HID transitions for proof and ordinary guest profiles.

use conduit_human::KeyEvent;

mod product;
pub use product::{
    ProductInputControl, ProductInputEvent, run_product, run_product_with_pointer, run_ps2_product,
};

use crate::{
    arch::{HidKeyTransition, HidKeyboardSession, UsbDevice, XhciReady},
    keyboard_bridge,
    keyboard_plan::PreparedKeyboardPlay,
    keyboard_play,
};

pub use crate::keyboard_ingress::{INGRESS_CAPACITY, KeyboardIngress, KeyboardIngressRefusal};

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
}
