//! Installed bounded normalized-pattern comparison.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};
use std::vec::Vec;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::COMPARE_PATTERN_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct PatternComparisonOperation {
    pending: Option<RequestId>,
    next_request: u32,
    received: [bool; 2],
    closed: [bool; 2],
    emitted: bool,
}

impl PatternComparisonOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(port @ 0..=1),
                value,
            } if self.pending.is_none() && !self.received[usize::from(port)] => {
                self.received[usize::from(port)] = true;
                let request = RequestId(self.next_request);
                self.next_request = match self.next_request.checked_add(1) {
                    Some(next) => next,
                    None => return fail(FailureCode::StorageExhausted, 253),
                };
                self.pending = Some(request);
                let Ok(input) =
                    BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
                else {
                    self.pending = None;
                    return fail(FailureCode::InvalidInput, 254);
                };
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(port),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None)
                        if self.received == [true, true] && !self.emitted =>
                    {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Completed, None, None) => OperationAction::Await,
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 0),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 250),
                }
            }
            OperationInput::Closed {
                port: PortId(port @ 0..=1),
            } if self.pending.is_none() && !self.closed[usize::from(port)] => {
                self.closed[usize::from(port)] = true;
                if self.closed == [true, true] && self.emitted {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 251),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.closed == [true, true] && self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

pub(super) struct PatternComparisonHost {
    tolerance: u64,
    type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    candidate: Vec<u8>,
    template: Vec<u8>,
    output: Vec<u8>,
}

impl PatternComparisonHost {
    pub(super) fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        let tolerance = validate(placement)?;
        Ok(Self {
            tolerance,
            type_prefix: conduit_semantic_catalog::normalized_duration_sequence_type()
                .canonical_bytes()
                .map_err(|error| format!("normalized type: {error:?}"))?,
            output_type_prefix: conduit_semantic_catalog::pattern_comparison_type()
                .canonical_bytes()
                .map_err(|error| format!("comparison type: {error:?}"))?,
            candidate: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            template: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    pub(super) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<Option<&[u8]>, conduit_semantic_catalog::PatternComparisonRefusal> {
        let target = match contract {
            conduit_std_offers::COMPARE_PATTERN_CANDIDATE_OPERATION => &mut self.candidate,
            conduit_std_offers::COMPARE_PATTERN_TEMPLATE_OPERATION => &mut self.template,
            _ => return Err(conduit_semantic_catalog::PatternComparisonRefusal::Malformed),
        };
        if !target.is_empty() || input.len() > target.capacity() {
            return Err(conduit_semantic_catalog::PatternComparisonRefusal::Malformed);
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

fn decode<'a>(
    input: &'a [u8],
    prefix: &[u8],
) -> Result<&'a [u8], conduit_semantic_catalog::PatternComparisonRefusal> {
    use conduit_semantic_catalog::PatternComparisonRefusal::*;
    let mut input = input.strip_prefix(prefix).ok_or(Malformed)?;
    if take_byte(&mut input)? != 2 || take_u32(&mut input)? != 2 {
        return Err(Malformed);
    }
    let algorithm = take_named_leaf(&mut input, "algorithm")?;
    let values = take_named_leaf(&mut input, "values")?;
    if algorithm != conduit_semantic_catalog::NORMALIZATION_ALGORITHM.as_bytes()
        || !input.is_empty()
    {
        return Err(AlgorithmMismatch);
    }
    inspect_values(values)?;
    Ok(values)
}

fn inspect_values(
    values: &[u8],
) -> Result<usize, conduit_semantic_catalog::PatternComparisonRefusal> {
    use conduit_semantic_catalog::PatternComparisonRefusal::Malformed;
    if values.is_empty() {
        return Err(Malformed);
    }
    let mut count = 0;
    for raw in values.split(|byte| *byte == b',') {
        count += 1;
        if count >= conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS {
            return Err(Malformed);
        }
        let value = parse_u64(raw)?;
        if value > conduit_semantic_catalog::NORMALIZED_SCALE {
            return Err(Malformed);
        }
    }
    Ok(count)
}

fn compare(
    candidate: &[u8],
    template: &[u8],
) -> Result<u64, conduit_semantic_catalog::PatternComparisonRefusal> {
    use conduit_semantic_catalog::PatternComparisonRefusal::LengthMismatch;
    if inspect_values(candidate)? != inspect_values(template)? {
        return Err(LengthMismatch);
    }
    let maximum_error = candidate
        .split(|byte| *byte == b',')
        .zip(template.split(|byte| *byte == b','))
        .try_fold(0_u64, |maximum, (candidate, template)| {
            Ok::<_, conduit_semantic_catalog::PatternComparisonRefusal>(
                maximum.max(parse_u64(candidate)?.abs_diff(parse_u64(template)?)),
            )
        })?;
    Ok(conduit_semantic_catalog::NORMALIZED_SCALE.saturating_sub(maximum_error))
}

fn encode_result(
    output: &mut Vec<u8>,
    prefix: &[u8],
    tolerance: u64,
    score: u64,
) -> Result<(), conduit_semantic_catalog::PatternComparisonRefusal> {
    output.clear();
    output.extend_from_slice(prefix);
    output.push(2);
    output.extend_from_slice(&4_u32.to_le_bytes());
    field_leaf(
        output,
        "matched",
        if score >= conduit_semantic_catalog::NORMALIZED_SCALE - tolerance {
            b"true"
        } else {
            b"false"
        },
    );
    field_leaf(
        output,
        "metric",
        conduit_semantic_catalog::MAXIMUM_ABSOLUTE_METRIC.as_bytes(),
    );
    field_u64(output, "score_millionths", score);
    field_u64(output, "tolerance_millionths", tolerance);
    (output.len() <= MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .then_some(())
        .ok_or(conduit_semantic_catalog::PatternComparisonRefusal::Malformed)
}

fn parse_u64(raw: &[u8]) -> Result<u64, conduit_semantic_catalog::PatternComparisonRefusal> {
    use conduit_semantic_catalog::PatternComparisonRefusal::Malformed;
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
) -> Result<&'a [u8], conduit_semantic_catalog::PatternComparisonRefusal> {
    use conduit_semantic_catalog::PatternComparisonRefusal::Malformed;
    if take_bytes(input)? != name.as_bytes() || take_byte(input)? != 0 {
        return Err(Malformed);
    }
    take_bytes(input)
}
fn take_byte(input: &mut &[u8]) -> Result<u8, conduit_semantic_catalog::PatternComparisonRefusal> {
    let (&value, rest) = input
        .split_first()
        .ok_or(conduit_semantic_catalog::PatternComparisonRefusal::Malformed)?;
    *input = rest;
    Ok(value)
}
fn take_u32(input: &mut &[u8]) -> Result<u32, conduit_semantic_catalog::PatternComparisonRefusal> {
    let raw: [u8; 4] = input
        .get(..4)
        .ok_or(conduit_semantic_catalog::PatternComparisonRefusal::Malformed)?
        .try_into()
        .map_err(|_| conduit_semantic_catalog::PatternComparisonRefusal::Malformed)?;
    *input = &input[4..];
    Ok(u32::from_le_bytes(raw))
}
fn take_bytes<'a>(
    input: &mut &'a [u8],
) -> Result<&'a [u8], conduit_semantic_catalog::PatternComparisonRefusal> {
    let length = usize::try_from(take_u32(input)?)
        .map_err(|_| conduit_semantic_catalog::PatternComparisonRefusal::Malformed)?;
    let value = input
        .get(..length)
        .ok_or(conduit_semantic_catalog::PatternComparisonRefusal::Malformed)?;
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

pub(super) fn refusal_detail(refusal: &conduit_semantic_catalog::PatternComparisonRefusal) -> u16 {
    use conduit_semantic_catalog::PatternComparisonRefusal::*;
    match refusal {
        Malformed => 1,
        UnsupportedMetric => 2,
        ToleranceOutOfRange => 3,
        AlgorithmMismatch => 4,
        LengthMismatch => 5,
    }
}

fn validate(placement: &PlannedGear) -> Result<u64, String> {
    let offer = conduit_std_offers::compare_pattern_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
    {
        return Err("planned pattern comparison differs from installed realization".into());
    }
    let metric = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("metric", ConfigurationValue::Text(value)) => Some(value.as_str()),
            _ => None,
        })
        .ok_or("comparison metric is absent")?;
    if metric != conduit_semantic_catalog::MAXIMUM_ABSOLUTE_METRIC {
        return Err("comparison metric is unsupported".into());
    }
    let tolerance = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("tolerance-millionths", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or("comparison tolerance is absent")?;
    if tolerance > conduit_semantic_catalog::NORMALIZED_SCALE {
        return Err("comparison tolerance is outside reviewed bounds".into());
    }
    Ok(tolerance)
}
fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 3,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 3) as u32,
        host_requests: 2,
        sign_items: 24,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    budget(placement)?;
    Ok(InstalledOperation::PatternComparison(
        PatternComparisonOperation {
            pending: None,
            next_request: 0,
            received: [false; 2],
            closed: [false; 2],
            emitted: false,
        },
    ))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
