//! Preallocated canonical timing transform shared by Host realizations.
//! Prepare before Play; execution reuses the admitted output buffer.
//! The calling Host enforces its planned input-byte bound before execution,
//! as it does for every admitted structured Host-operation request.

use alloc::vec::Vec;
use conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;

pub struct BoundedIntervalCodec {
    input_type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    output: Vec<u8>,
}

impl BoundedIntervalCodec {
    pub fn prepare() -> Self {
        Self {
            input_type_prefix: crate::timed_event_sequence_type()
                .canonical_bytes()
                .expect("installed timed-event type is canonical"),
            output_type_prefix: crate::interval_sequence_type()
                .canonical_bytes()
                .expect("installed interval type is canonical"),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    pub fn execute(&mut self, input: &[u8]) -> Result<&[u8], crate::TimedPatternRefusal> {
        let (clock_basis, event_times) = decode_input(input, &self.input_type_prefix)?;
        encode_output(
            &mut self.output,
            &self.output_type_prefix,
            clock_basis,
            event_times,
        )?;
        Ok(&self.output)
    }
}

fn decode_input<'a>(
    input: &'a [u8],
    type_prefix: &[u8],
) -> Result<(&'a [u8], &'a [u8]), crate::TimedPatternRefusal> {
    let mut input = input
        .strip_prefix(type_prefix)
        .ok_or(crate::TimedPatternRefusal::Malformed)?;
    if take_byte(&mut input)? != 2 || take_u32(&mut input)? != 2 {
        return Err(crate::TimedPatternRefusal::Malformed);
    }
    let clock_basis = take_named_leaf(&mut input, "clock_basis")?;
    let event_times = take_named_leaf(&mut input, "event_times")?;
    if clock_basis.is_empty() || !input.is_empty() {
        return Err(crate::TimedPatternRefusal::Malformed);
    }
    core::str::from_utf8(clock_basis).map_err(|_| crate::TimedPatternRefusal::Malformed)?;
    Ok((clock_basis, event_times))
}

fn encode_output(
    output: &mut Vec<u8>,
    type_prefix: &[u8],
    clock_basis: &[u8],
    event_times: &[u8],
) -> Result<(), crate::TimedPatternRefusal> {
    use crate::TimedPatternRefusal;

    output.clear();
    output.extend_from_slice(type_prefix);
    output.push(2);
    output.extend_from_slice(&2_u32.to_le_bytes());
    field_leaf(output, "clock_basis", clock_basis);
    bytes(output, b"intervals");
    output.push(0);
    let length_at = output.len();
    output.extend_from_slice(&0_u32.to_le_bytes());
    let intervals_at = output.len();

    let mut count = 0_usize;
    let mut previous = None;
    for raw in event_times.split(|byte| *byte == b',') {
        if raw.is_empty() {
            return Err(TimedPatternRefusal::Malformed);
        }
        count += 1;
        if count > crate::MAXIMUM_TIMED_EVENTS {
            return Err(TimedPatternRefusal::TooManyEvents);
        }
        let current = parse_u64(raw)?;
        if let Some(previous) = previous {
            if current <= previous {
                return Err(TimedPatternRefusal::ReorderedOrDuplicateEvent);
            }
            if output.len() != intervals_at {
                output.push(b',');
            }
            append_digits(
                output,
                current
                    .checked_sub(previous)
                    .ok_or(TimedPatternRefusal::IntervalOverflow)?,
            );
        }
        previous = Some(current);
    }
    if count < 2 {
        return Err(TimedPatternRefusal::TooFewEvents);
    }
    let length =
        u32::try_from(output.len() - intervals_at).map_err(|_| TimedPatternRefusal::Malformed)?;
    output[length_at..length_at + 4].copy_from_slice(&length.to_le_bytes());
    (output.len() <= MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .then_some(())
        .ok_or(TimedPatternRefusal::Malformed)
}

fn parse_u64(raw: &[u8]) -> Result<u64, crate::TimedPatternRefusal> {
    let mut value = 0_u64;
    for digit in raw {
        if !digit.is_ascii_digit() {
            return Err(crate::TimedPatternRefusal::Malformed);
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(*digit - b'0')))
            .ok_or(crate::TimedPatternRefusal::Malformed)?;
    }
    Ok(value)
}

fn take_named_leaf<'a>(
    input: &mut &'a [u8],
    name: &str,
) -> Result<&'a [u8], crate::TimedPatternRefusal> {
    if take_bytes(input)? != name.as_bytes() || take_byte(input)? != 0 {
        return Err(crate::TimedPatternRefusal::Malformed);
    }
    take_bytes(input)
}

fn take_byte(input: &mut &[u8]) -> Result<u8, crate::TimedPatternRefusal> {
    let (&value, rest) = input
        .split_first()
        .ok_or(crate::TimedPatternRefusal::Malformed)?;
    *input = rest;
    Ok(value)
}

fn take_u32(input: &mut &[u8]) -> Result<u32, crate::TimedPatternRefusal> {
    let raw: [u8; 4] = input
        .get(..4)
        .ok_or(crate::TimedPatternRefusal::Malformed)?
        .try_into()
        .map_err(|_| crate::TimedPatternRefusal::Malformed)?;
    *input = &input[4..];
    Ok(u32::from_le_bytes(raw))
}

fn take_bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], crate::TimedPatternRefusal> {
    let length =
        usize::try_from(take_u32(input)?).map_err(|_| crate::TimedPatternRefusal::Malformed)?;
    let value = input
        .get(..length)
        .ok_or(crate::TimedPatternRefusal::Malformed)?;
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
