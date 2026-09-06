//! Bounded canonical pattern comparison shared by compatible Hosts.
use alloc::{format, string::String, vec::Vec};
use conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternComparisonInput {
    Candidate,
    Template,
}

pub struct BoundedPatternComparisonCodec {
    tolerance: u64,
    type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    candidate: Vec<u8>,
    template: Vec<u8>,
    output: Vec<u8>,
}

impl BoundedPatternComparisonCodec {
    pub fn new(tolerance: u64) -> Result<Self, String> {
        if tolerance > crate::NORMALIZED_SCALE {
            return Err("pattern tolerance exceeds normalized scale".into());
        }
        Ok(Self {
            tolerance,
            type_prefix: crate::normalized_duration_sequence_type()
                .canonical_bytes()
                .map_err(|error| format!("normalized type: {error:?}"))?,
            output_type_prefix: crate::pattern_comparison_type()
                .canonical_bytes()
                .map_err(|error| format!("comparison type: {error:?}"))?,
            candidate: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            template: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    pub fn execute(
        &mut self,
        port: PatternComparisonInput,
        input: &[u8],
    ) -> Result<Option<&[u8]>, crate::PatternComparisonRefusal> {
        let target = match port {
            PatternComparisonInput::Candidate => &mut self.candidate,
            PatternComparisonInput::Template => &mut self.template,
        };
        if !target.is_empty() || input.len() > target.capacity() {
            return Err(crate::PatternComparisonRefusal::Malformed);
        }
        target.extend_from_slice(input);
        if self.candidate.is_empty() || self.template.is_empty() {
            return Ok(None);
        }
        let candidate = decode(&self.candidate, &self.type_prefix)?;
        let template = decode(&self.template, &self.type_prefix)?;
        let score = compare(candidate, template)?;
        encode_result(
            &mut self.output,
            &self.output_type_prefix,
            self.tolerance,
            score,
        )?;
        Ok(Some(&self.output))
    }
}

fn decode<'a>(input: &'a [u8], prefix: &[u8]) -> Result<&'a [u8], crate::PatternComparisonRefusal> {
    use crate::PatternComparisonRefusal::*;
    let mut input = input.strip_prefix(prefix).ok_or(Malformed)?;
    if take_byte(&mut input)? != 2 || take_u32(&mut input)? != 2 {
        return Err(Malformed);
    }
    let algorithm = take_named_leaf(&mut input, "algorithm")?;
    let values = take_named_leaf(&mut input, "values")?;
    if algorithm != crate::NORMALIZATION_ALGORITHM.as_bytes() || !input.is_empty() {
        return Err(AlgorithmMismatch);
    }
    inspect_values(values)?;
    Ok(values)
}

fn inspect_values(values: &[u8]) -> Result<usize, crate::PatternComparisonRefusal> {
    use crate::PatternComparisonRefusal::Malformed;
    if values.is_empty() {
        return Err(Malformed);
    }
    let mut count = 0;
    for raw in values.split(|byte| *byte == b',') {
        count += 1;
        if count >= crate::MAXIMUM_TIMED_EVENTS {
            return Err(Malformed);
        }
        let value = parse_u64(raw)?;
        if value > crate::NORMALIZED_SCALE {
            return Err(Malformed);
        }
    }
    Ok(count)
}

fn compare(candidate: &[u8], template: &[u8]) -> Result<u64, crate::PatternComparisonRefusal> {
    use crate::PatternComparisonRefusal::LengthMismatch;
    if inspect_values(candidate)? != inspect_values(template)? {
        return Err(LengthMismatch);
    }
    let maximum_error = candidate
        .split(|byte| *byte == b',')
        .zip(template.split(|byte| *byte == b','))
        .try_fold(0_u64, |maximum, (candidate, template)| {
            Ok::<_, crate::PatternComparisonRefusal>(
                maximum.max(parse_u64(candidate)?.abs_diff(parse_u64(template)?)),
            )
        })?;
    Ok(crate::NORMALIZED_SCALE.saturating_sub(maximum_error))
}

fn encode_result(
    output: &mut Vec<u8>,
    prefix: &[u8],
    tolerance: u64,
    score: u64,
) -> Result<(), crate::PatternComparisonRefusal> {
    output.clear();
    output.extend_from_slice(prefix);
    output.push(2);
    output.extend_from_slice(&4_u32.to_le_bytes());
    field_leaf(
        output,
        "matched",
        if score >= crate::NORMALIZED_SCALE - tolerance {
            b"true"
        } else {
            b"false"
        },
    );
    field_leaf(output, "metric", crate::MAXIMUM_ABSOLUTE_METRIC.as_bytes());
    field_u64(output, "score_millionths", score);
    field_u64(output, "tolerance_millionths", tolerance);
    (output.len() <= MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .then_some(())
        .ok_or(crate::PatternComparisonRefusal::Malformed)
}

fn parse_u64(raw: &[u8]) -> Result<u64, crate::PatternComparisonRefusal> {
    use crate::PatternComparisonRefusal::Malformed;
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
) -> Result<&'a [u8], crate::PatternComparisonRefusal> {
    use crate::PatternComparisonRefusal::Malformed;
    if take_bytes(input)? != name.as_bytes() || take_byte(input)? != 0 {
        return Err(Malformed);
    }
    take_bytes(input)
}
fn take_byte(input: &mut &[u8]) -> Result<u8, crate::PatternComparisonRefusal> {
    let (&value, rest) = input
        .split_first()
        .ok_or(crate::PatternComparisonRefusal::Malformed)?;
    *input = rest;
    Ok(value)
}
fn take_u32(input: &mut &[u8]) -> Result<u32, crate::PatternComparisonRefusal> {
    let raw: [u8; 4] = input
        .get(..4)
        .ok_or(crate::PatternComparisonRefusal::Malformed)?
        .try_into()
        .map_err(|_| crate::PatternComparisonRefusal::Malformed)?;
    *input = &input[4..];
    Ok(u32::from_le_bytes(raw))
}
fn take_bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], crate::PatternComparisonRefusal> {
    let length = usize::try_from(take_u32(input)?)
        .map_err(|_| crate::PatternComparisonRefusal::Malformed)?;
    let value = input
        .get(..length)
        .ok_or(crate::PatternComparisonRefusal::Malformed)?;
    *input = &input[length..];
    Ok(value)
}
fn bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}
fn field_leaf(output: &mut Vec<u8>, name: &str, value: &[u8]) {
    bytes(output, name.as_bytes());
    output.push(0);
    bytes(output, value);
}
fn field_u64(output: &mut Vec<u8>, name: &str, value: u64) {
    bytes(output, name.as_bytes());
    output.push(0);
    let at = output.len();
    output.extend_from_slice(&0_u32.to_le_bytes());
    let start = output.len();
    append_digits(output, value);
    let length = (output.len() - start) as u32;
    output[at..at + 4].copy_from_slice(&length.to_le_bytes());
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
