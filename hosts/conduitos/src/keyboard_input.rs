//! Consumers of validated HID transitions for proof and ordinary guest profiles.

use conduit_core::KeyEvent;

use crate::{
    arch::{HidKeyTransition, HidKeyboardSession, UsbDevice, XhciReady},
    keyboard_bridge,
    keyboard_plan::PreparedKeyboardPlay,
    keyboard_play,
};

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
}
