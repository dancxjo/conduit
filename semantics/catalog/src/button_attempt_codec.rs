//! Preallocated encoding of pressed-button instants supplied by an admitted clock.

use alloc::vec::Vec;
use conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;

#[derive(Debug)]
pub enum ButtonAttemptObservation<'a> {
    Released,
    Pressed,
    Complete(&'a [u8]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAttemptRefusal {
    MalformedTransition,
    TooManyEvents,
    ClockRegressed,
}

pub struct BoundedButtonAttemptCodec {
    input_type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    event_times: [u64; crate::MAXIMUM_TIMED_EVENTS],
    count: usize,
    maximum: usize,
    output: Vec<u8>,
}

impl BoundedButtonAttemptCodec {
    pub fn prepare(maximum: usize) -> Self {
        Self {
            input_type_prefix: crate::input_button_transition_type()
                .canonical_bytes()
                .expect("installed button-transition type is canonical"),
            output_type_prefix: crate::timed_event_sequence_type()
                .canonical_bytes()
                .expect("installed timed-event type is canonical"),
            event_times: [0; crate::MAXIMUM_TIMED_EVENTS],
            count: 0,
            maximum,
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    pub fn observe(
        &mut self,
        input: &[u8],
        now_micros: u64,
    ) -> Result<ButtonAttemptObservation<'_>, ButtonAttemptRefusal> {
        if !pressed_transition(input, &self.input_type_prefix)? {
            return Ok(ButtonAttemptObservation::Released);
        }
        if self.count >= self.maximum || self.count >= self.event_times.len() {
            return Err(ButtonAttemptRefusal::TooManyEvents);
        }
        if self.count > 0 && now_micros <= self.event_times[self.count - 1] {
            return Err(ButtonAttemptRefusal::ClockRegressed);
        }
        self.event_times[self.count] = now_micros;
        self.count += 1;
        if self.count < self.maximum {
            return Ok(ButtonAttemptObservation::Pressed);
        }
        encode_sequence(
            &mut self.output,
            &self.output_type_prefix,
            &self.event_times[..self.count],
        );
        Ok(ButtonAttemptObservation::Complete(&self.output))
    }
}

fn pressed_transition(input: &[u8], type_prefix: &[u8]) -> Result<bool, ButtonAttemptRefusal> {
    let mut input = input
        .strip_prefix(type_prefix)
        .ok_or(ButtonAttemptRefusal::MalformedTransition)?;
    expect_byte(&mut input, 2)?;
    expect_u32(&mut input, 3)?;
    expect_bytes(&mut input, b"button_identity")?;
    expect_byte(&mut input, 0)?;
    if take_bytes(&mut input)?.is_empty() {
        return Err(ButtonAttemptRefusal::MalformedTransition);
    }
    expect_bytes(&mut input, b"phase")?;
    expect_byte(&mut input, 3)?;
    let phase = take_bytes(&mut input)?;
    if phase != b"pressed" && phase != b"released" {
        return Err(ButtonAttemptRefusal::MalformedTransition);
    }
    expect_byte(&mut input, 0)?;
    if !take_bytes(&mut input)?.is_empty() {
        return Err(ButtonAttemptRefusal::MalformedTransition);
    }
    expect_bytes(&mut input, b"sequence")?;
    expect_byte(&mut input, 0)?;
    let sequence = take_bytes(&mut input)?;
    if sequence.is_empty() || !sequence.iter().all(u8::is_ascii_digit) || !input.is_empty() {
        return Err(ButtonAttemptRefusal::MalformedTransition);
    }
    Ok(phase == b"pressed")
}

fn encode_sequence(output: &mut Vec<u8>, type_prefix: &[u8], times: &[u64]) {
    output.clear();
    output.extend_from_slice(type_prefix);
    output.push(2);
    output.extend_from_slice(&2_u32.to_le_bytes());
    field_leaf(output, "clock_basis", b"host/boot-monotonic-microseconds@1");
    bytes(output, b"event_times");
    output.push(0);
    let length_at = output.len();
    output.extend_from_slice(&0_u32.to_le_bytes());
    let values_at = output.len();
    for (index, value) in times.iter().copied().enumerate() {
        if index > 0 {
            output.push(b',');
        }
        append_digits(output, value);
    }
    let length = u32::try_from(output.len() - values_at).expect("bounded event encoding length");
    output[length_at..length_at + 4].copy_from_slice(&length.to_le_bytes());
}

fn expect_byte(input: &mut &[u8], expected: u8) -> Result<(), ButtonAttemptRefusal> {
    (take_byte(input)? == expected)
        .then_some(())
        .ok_or(ButtonAttemptRefusal::MalformedTransition)
}

fn take_byte(input: &mut &[u8]) -> Result<u8, ButtonAttemptRefusal> {
    let (&value, rest) = input
        .split_first()
        .ok_or(ButtonAttemptRefusal::MalformedTransition)?;
    *input = rest;
    Ok(value)
}

fn expect_u32(input: &mut &[u8], expected: u32) -> Result<(), ButtonAttemptRefusal> {
    (take_u32(input)? == expected)
        .then_some(())
        .ok_or(ButtonAttemptRefusal::MalformedTransition)
}

fn take_u32(input: &mut &[u8]) -> Result<u32, ButtonAttemptRefusal> {
    let raw: [u8; 4] = input
        .get(..4)
        .ok_or(ButtonAttemptRefusal::MalformedTransition)?
        .try_into()
        .map_err(|_| ButtonAttemptRefusal::MalformedTransition)?;
    *input = &input[4..];
    Ok(u32::from_le_bytes(raw))
}

fn expect_bytes(input: &mut &[u8], expected: &[u8]) -> Result<(), ButtonAttemptRefusal> {
    (take_bytes(input)? == expected)
        .then_some(())
        .ok_or(ButtonAttemptRefusal::MalformedTransition)
}

fn take_bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], ButtonAttemptRefusal> {
    let length =
        usize::try_from(take_u32(input)?).map_err(|_| ButtonAttemptRefusal::MalformedTransition)?;
    let value = input
        .get(..length)
        .ok_or(ButtonAttemptRefusal::MalformedTransition)?;
    *input = &input[length..];
    Ok(value)
}

fn field_leaf(output: &mut Vec<u8>, name: &str, value: &[u8]) {
    bytes(output, name.as_bytes());
    output.push(0);
    bytes(output, value);
}

fn bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}

fn append_digits(output: &mut Vec<u8>, mut value: u64) {
    let mut digits = [0_u8; 20];
    let mut cursor = digits.len();
    loop {
        cursor -= 1;
        digits[cursor] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    output.extend_from_slice(&digits[cursor..]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn transition(pressed: bool, sequence: u64) -> Vec<u8> {
        crate::button_transition_value("button/primary", pressed, sequence)
            .unwrap()
            .canonical_bytes()
            .unwrap()
    }

    #[test]
    fn releases_are_ignored_and_exact_press_count_completes() {
        let mut host = BoundedButtonAttemptCodec::prepare(3);
        assert!(matches!(
            host.observe(&transition(false, 1), 90),
            Ok(ButtonAttemptObservation::Released)
        ));
        assert!(matches!(
            host.observe(&transition(true, 2), 100),
            Ok(ButtonAttemptObservation::Pressed)
        ));
        assert!(matches!(
            host.observe(&transition(true, 3), 250),
            Ok(ButtonAttemptObservation::Pressed)
        ));
        let ButtonAttemptObservation::Complete(encoded) =
            host.observe(&transition(true, 4), 700).unwrap()
        else {
            panic!("third press must complete the admitted attempt");
        };
        let value = conduit_core::StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
        let intervals = crate::derive_intervals(&value).unwrap();
        assert_eq!(
            crate::decode_intervals(&intervals).unwrap(),
            ("host/boot-monotonic-microseconds@1".into(), vec![150, 450])
        );
    }

    #[test]
    fn malformed_overflow_and_stale_clock_are_distinct() {
        let mut host = BoundedButtonAttemptCodec::prepare(2);
        assert_eq!(
            host.observe(b"bad", 1).unwrap_err(),
            ButtonAttemptRefusal::MalformedTransition
        );
        assert!(matches!(
            host.observe(&transition(true, 1), 10),
            Ok(ButtonAttemptObservation::Pressed)
        ));
        assert_eq!(
            host.observe(&transition(true, 2), 10).unwrap_err(),
            ButtonAttemptRefusal::ClockRegressed
        );
        let mut one = BoundedButtonAttemptCodec::prepare(1);
        assert!(matches!(
            one.observe(&transition(true, 1), 1),
            Ok(ButtonAttemptObservation::Complete(_))
        ));
        assert_eq!(
            one.observe(&transition(true, 2), 2).unwrap_err(),
            ButtonAttemptRefusal::TooManyEvents
        );
    }
}
