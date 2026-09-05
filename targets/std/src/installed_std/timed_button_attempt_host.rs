//! Preallocated hosted observation of pressed-button instants.

use conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;
use std::vec::Vec;

#[derive(Debug)]
pub(super) enum Observation<'a> {
    Released,
    Pressed,
    Complete(&'a [u8]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refusal {
    MalformedTransition,
    TooManyEvents,
    ClockRegressed,
}

pub(super) struct TimedButtonAttemptHost {
    input_type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    event_times: [u64; conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS],
    count: usize,
    maximum: usize,
    output: Vec<u8>,
}

impl TimedButtonAttemptHost {
    pub(super) fn prepare(maximum: usize) -> Self {
        Self {
            input_type_prefix: conduit_semantic_catalog::input_button_transition_type()
                .canonical_bytes()
                .expect("installed button-transition type is canonical"),
            output_type_prefix: conduit_semantic_catalog::timed_event_sequence_type()
                .canonical_bytes()
                .expect("installed timed-event type is canonical"),
            event_times: [0; conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS],
            count: 0,
            maximum,
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    pub(super) fn observe(
        &mut self,
        input: &[u8],
        now_micros: u64,
    ) -> Result<Observation<'_>, Refusal> {
        if !pressed_transition(input, &self.input_type_prefix)? {
            return Ok(Observation::Released);
        }
        if self.count >= self.maximum || self.count >= self.event_times.len() {
            return Err(Refusal::TooManyEvents);
        }
        if self.count > 0 && now_micros <= self.event_times[self.count - 1] {
            return Err(Refusal::ClockRegressed);
        }
        self.event_times[self.count] = now_micros;
        self.count += 1;
        if self.count < self.maximum {
            return Ok(Observation::Pressed);
        }
        encode_sequence(
            &mut self.output,
            &self.output_type_prefix,
            &self.event_times[..self.count],
        );
        Ok(Observation::Complete(&self.output))
    }
}

fn pressed_transition(input: &[u8], type_prefix: &[u8]) -> Result<bool, Refusal> {
    let mut input = input
        .strip_prefix(type_prefix)
        .ok_or(Refusal::MalformedTransition)?;
    expect_byte(&mut input, 2)?;
    expect_u32(&mut input, 3)?;
    expect_bytes(&mut input, b"button_identity")?;
    expect_byte(&mut input, 0)?;
    if take_bytes(&mut input)?.is_empty() {
        return Err(Refusal::MalformedTransition);
    }
    expect_bytes(&mut input, b"phase")?;
    expect_byte(&mut input, 3)?;
    let phase = take_bytes(&mut input)?;
    if phase != b"pressed" && phase != b"released" {
        return Err(Refusal::MalformedTransition);
    }
    expect_byte(&mut input, 0)?;
    if !take_bytes(&mut input)?.is_empty() {
        return Err(Refusal::MalformedTransition);
    }
    expect_bytes(&mut input, b"sequence")?;
    expect_byte(&mut input, 0)?;
    let sequence = take_bytes(&mut input)?;
    if sequence.is_empty() || !sequence.iter().all(u8::is_ascii_digit) || !input.is_empty() {
        return Err(Refusal::MalformedTransition);
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

fn expect_byte(input: &mut &[u8], expected: u8) -> Result<(), Refusal> {
    (take_byte(input)? == expected)
        .then_some(())
        .ok_or(Refusal::MalformedTransition)
}

fn take_byte(input: &mut &[u8]) -> Result<u8, Refusal> {
    let (&value, rest) = input.split_first().ok_or(Refusal::MalformedTransition)?;
    *input = rest;
    Ok(value)
}

fn expect_u32(input: &mut &[u8], expected: u32) -> Result<(), Refusal> {
    (take_u32(input)? == expected)
        .then_some(())
        .ok_or(Refusal::MalformedTransition)
}

fn take_u32(input: &mut &[u8]) -> Result<u32, Refusal> {
    let raw: [u8; 4] = input
        .get(..4)
        .ok_or(Refusal::MalformedTransition)?
        .try_into()
        .map_err(|_| Refusal::MalformedTransition)?;
    *input = &input[4..];
    Ok(u32::from_le_bytes(raw))
}

fn expect_bytes(input: &mut &[u8], expected: &[u8]) -> Result<(), Refusal> {
    (take_bytes(input)? == expected)
        .then_some(())
        .ok_or(Refusal::MalformedTransition)
}

fn take_bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], Refusal> {
    let length = usize::try_from(take_u32(input)?).map_err(|_| Refusal::MalformedTransition)?;
    let value = input.get(..length).ok_or(Refusal::MalformedTransition)?;
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

    fn transition(pressed: bool, sequence: u64) -> Vec<u8> {
        conduit_semantic_catalog::button_transition_value("button/primary", pressed, sequence)
            .unwrap()
            .canonical_bytes()
            .unwrap()
    }

    #[test]
    fn releases_are_ignored_and_exact_press_count_completes() {
        let mut host = TimedButtonAttemptHost::prepare(3);
        assert!(matches!(
            host.observe(&transition(false, 1), 90),
            Ok(Observation::Released)
        ));
        assert!(matches!(
            host.observe(&transition(true, 2), 100),
            Ok(Observation::Pressed)
        ));
        assert!(matches!(
            host.observe(&transition(true, 3), 250),
            Ok(Observation::Pressed)
        ));
        let Observation::Complete(encoded) = host.observe(&transition(true, 4), 700).unwrap()
        else {
            panic!("third press must complete the admitted attempt");
        };
        let value = conduit_core::StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
        let intervals = conduit_semantic_catalog::derive_intervals(&value).unwrap();
        assert_eq!(
            conduit_semantic_catalog::decode_intervals(&intervals).unwrap(),
            ("host/boot-monotonic-microseconds@1".into(), vec![150, 450])
        );
    }

    #[test]
    fn malformed_overflow_and_stale_clock_are_distinct() {
        let mut host = TimedButtonAttemptHost::prepare(2);
        assert_eq!(
            host.observe(b"bad", 1).unwrap_err(),
            Refusal::MalformedTransition
        );
        assert!(matches!(
            host.observe(&transition(true, 1), 10),
            Ok(Observation::Pressed)
        ));
        assert_eq!(
            host.observe(&transition(true, 2), 10).unwrap_err(),
            Refusal::ClockRegressed
        );
        let mut one = TimedButtonAttemptHost::prepare(1);
        assert!(matches!(
            one.observe(&transition(true, 1), 1),
            Ok(Observation::Complete(_))
        ));
        assert_eq!(
            one.observe(&transition(true, 2), 2).unwrap_err(),
            Refusal::TooManyEvents
        );
    }
}
