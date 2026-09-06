//! Preallocated canonical timing transform shared by Host realizations.
//! Prepare before Play; execution reuses the admitted output buffer.
//! The calling Host enforces its planned input-byte bound before execution,
//! as it does for every admitted structured Host-operation request.

use alloc::vec::Vec;
use conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;

pub struct BoundedNormalizationCodec {
    input_type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    output: Vec<u8>,
}

impl BoundedNormalizationCodec {
    pub fn prepare() -> Self {
        Self {
            input_type_prefix: crate::interval_sequence_type()
                .canonical_bytes()
                .expect("installed interval type is canonical"),
            output_type_prefix: crate::normalized_duration_sequence_type()
                .canonical_bytes()
                .expect("installed normalized sequence type is canonical"),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    pub fn execute(&mut self, input: &[u8]) -> Result<&[u8], crate::SequenceNormalizationRefusal> {
        let intervals = decode_intervals(input, &self.input_type_prefix)?;
        encode_normalized(&mut self.output, &self.output_type_prefix, intervals)?;
        Ok(&self.output)
    }
}

fn decode_intervals<'a>(
    input: &'a [u8],
    type_prefix: &[u8],
) -> Result<&'a [u8], crate::SequenceNormalizationRefusal> {
    use crate::SequenceNormalizationRefusal::Malformed;
    let mut input = input.strip_prefix(type_prefix).ok_or(Malformed)?;
    if take_byte(&mut input)? != 2 || take_u32(&mut input)? != 2 {
        return Err(Malformed);
    }
    let clock_basis = take_named_leaf(&mut input, "clock_basis")?;
    let intervals = take_named_leaf(&mut input, "intervals")?;
    if clock_basis.is_empty() || core::str::from_utf8(clock_basis).is_err() || !input.is_empty() {
        return Err(Malformed);
    }
    Ok(intervals)
}

fn encode_normalized(
    output: &mut Vec<u8>,
    type_prefix: &[u8],
    intervals: &[u8],
) -> Result<(), crate::SequenceNormalizationRefusal> {
    use crate::SequenceNormalizationRefusal;
    let (count, maximum) = inspect_intervals(intervals)?;
    output.clear();
    output.extend_from_slice(type_prefix);
    output.push(2);
    output.extend_from_slice(&2_u32.to_le_bytes());
    field_leaf(
        output,
        "algorithm",
        crate::NORMALIZATION_ALGORITHM.as_bytes(),
    );
    bytes(output, b"values");
    output.push(0);
    let length_at = output.len();
    output.extend_from_slice(&0_u32.to_le_bytes());
    let values_at = output.len();
    for (index, raw) in intervals.split(|byte| *byte == b',').enumerate() {
        if index > 0 {
            output.push(b',');
        }
        let value = parse_u64(raw)?;
        let numerator = u128::from(value) * u128::from(crate::NORMALIZED_SCALE);
        let normalized = ((numerator + u128::from(maximum / 2)) / u128::from(maximum)) as u64;
        append_digits(output, normalized);
    }
    debug_assert!(count > 0);
    let length = u32::try_from(output.len() - values_at)
        .map_err(|_| SequenceNormalizationRefusal::Malformed)?;
    output[length_at..length_at + 4].copy_from_slice(&length.to_le_bytes());
    (output.len() <= MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .then_some(())
        .ok_or(SequenceNormalizationRefusal::Malformed)
}

fn inspect_intervals(
    intervals: &[u8],
) -> Result<(usize, u64), crate::SequenceNormalizationRefusal> {
    use crate::SequenceNormalizationRefusal;
    if intervals.is_empty() {
        return Err(SequenceNormalizationRefusal::Empty);
    }
    let mut count = 0;
    let mut maximum = 0;
    for raw in intervals.split(|byte| *byte == b',') {
        count += 1;
        if count >= crate::MAXIMUM_TIMED_EVENTS {
            return Err(SequenceNormalizationRefusal::TooManyValues);
        }
        let value = parse_u64(raw)?;
        if value == 0 {
            return Err(SequenceNormalizationRefusal::ZeroDuration);
        }
        maximum = maximum.max(value);
    }
    Ok((count, maximum))
}

fn parse_u64(raw: &[u8]) -> Result<u64, crate::SequenceNormalizationRefusal> {
    use crate::SequenceNormalizationRefusal::Malformed;
    if raw.is_empty() {
        return Err(Malformed);
    }
    raw.iter().try_fold(0_u64, |value, digit| {
        if !digit.is_ascii_digit() {
            return Err(Malformed);
        }
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(*digit - b'0')))
            .ok_or(Malformed)
    })
}

fn take_named_leaf<'a>(
    input: &mut &'a [u8],
    name: &str,
) -> Result<&'a [u8], crate::SequenceNormalizationRefusal> {
    use crate::SequenceNormalizationRefusal::Malformed;
    if take_bytes(input)? != name.as_bytes() || take_byte(input)? != 0 {
        return Err(Malformed);
    }
    take_bytes(input)
}

fn take_byte(input: &mut &[u8]) -> Result<u8, crate::SequenceNormalizationRefusal> {
    let (&value, rest) = input
        .split_first()
        .ok_or(crate::SequenceNormalizationRefusal::Malformed)?;
    *input = rest;
    Ok(value)
}

fn take_u32(input: &mut &[u8]) -> Result<u32, crate::SequenceNormalizationRefusal> {
    let raw: [u8; 4] = input
        .get(..4)
        .ok_or(crate::SequenceNormalizationRefusal::Malformed)?
        .try_into()
        .map_err(|_| crate::SequenceNormalizationRefusal::Malformed)?;
    *input = &input[4..];
    Ok(u32::from_le_bytes(raw))
}

fn take_bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], crate::SequenceNormalizationRefusal> {
    let length = usize::try_from(take_u32(input)?)
        .map_err(|_| crate::SequenceNormalizationRefusal::Malformed)?;
    let value = input
        .get(..length)
        .ok_or(crate::SequenceNormalizationRefusal::Malformed)?;
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
