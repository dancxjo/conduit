//! Installed bounded relative-duration normalization.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};
use std::vec::Vec;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::NORMALIZE_SEQUENCE_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct SequenceNormalizationOperation {
    pending: bool,
    completed: bool,
}

impl SequenceNormalizationOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.completed => {
                self.pending = true;
                let Ok(input) =
                    BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
                else {
                    return InstalledOperation::fail(240);
                };
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending => {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.completed = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Cancelled, _, _) => OperationAction::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    }),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => InstalledOperation::fail(241),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.completed && !self.pending => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(242),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.completed {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }
}

pub(super) struct SequenceNormalizationHost {
    input_type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    output: Vec<u8>,
}

impl SequenceNormalizationHost {
    pub(super) fn prepare() -> Self {
        Self {
            input_type_prefix: conduit_semantic_catalog::interval_sequence_type()
                .canonical_bytes()
                .expect("installed interval type is canonical"),
            output_type_prefix: conduit_semantic_catalog::normalized_duration_sequence_type()
                .canonical_bytes()
                .expect("installed normalized sequence type is canonical"),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    pub(super) fn execute(
        &mut self,
        input: &[u8],
    ) -> Result<&[u8], conduit_semantic_catalog::SequenceNormalizationRefusal> {
        let intervals = decode_intervals(input, &self.input_type_prefix)?;
        encode_normalized(&mut self.output, &self.output_type_prefix, intervals)?;
        Ok(&self.output)
    }
}

fn decode_intervals<'a>(
    input: &'a [u8],
    type_prefix: &[u8],
) -> Result<&'a [u8], conduit_semantic_catalog::SequenceNormalizationRefusal> {
    use conduit_semantic_catalog::SequenceNormalizationRefusal::Malformed;
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
) -> Result<(), conduit_semantic_catalog::SequenceNormalizationRefusal> {
    use conduit_semantic_catalog::SequenceNormalizationRefusal;
    let (count, maximum) = inspect_intervals(intervals)?;
    output.clear();
    output.extend_from_slice(type_prefix);
    output.push(2);
    output.extend_from_slice(&2_u32.to_le_bytes());
    field_leaf(
        output,
        "algorithm",
        conduit_semantic_catalog::NORMALIZATION_ALGORITHM.as_bytes(),
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
        let numerator = u128::from(value) * u128::from(conduit_semantic_catalog::NORMALIZED_SCALE);
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
) -> Result<(usize, u64), conduit_semantic_catalog::SequenceNormalizationRefusal> {
    use conduit_semantic_catalog::SequenceNormalizationRefusal;
    if intervals.is_empty() {
        return Err(SequenceNormalizationRefusal::Empty);
    }
    let mut count = 0;
    let mut maximum = 0;
    for raw in intervals.split(|byte| *byte == b',') {
        count += 1;
        if count >= conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS {
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

fn parse_u64(raw: &[u8]) -> Result<u64, conduit_semantic_catalog::SequenceNormalizationRefusal> {
    use conduit_semantic_catalog::SequenceNormalizationRefusal::Malformed;
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
) -> Result<&'a [u8], conduit_semantic_catalog::SequenceNormalizationRefusal> {
    use conduit_semantic_catalog::SequenceNormalizationRefusal::Malformed;
    if take_bytes(input)? != name.as_bytes() || take_byte(input)? != 0 {
        return Err(Malformed);
    }
    take_bytes(input)
}

fn take_byte(
    input: &mut &[u8],
) -> Result<u8, conduit_semantic_catalog::SequenceNormalizationRefusal> {
    let (&value, rest) = input
        .split_first()
        .ok_or(conduit_semantic_catalog::SequenceNormalizationRefusal::Malformed)?;
    *input = rest;
    Ok(value)
}

fn take_u32(
    input: &mut &[u8],
) -> Result<u32, conduit_semantic_catalog::SequenceNormalizationRefusal> {
    let raw: [u8; 4] = input
        .get(..4)
        .ok_or(conduit_semantic_catalog::SequenceNormalizationRefusal::Malformed)?
        .try_into()
        .map_err(|_| conduit_semantic_catalog::SequenceNormalizationRefusal::Malformed)?;
    *input = &input[4..];
    Ok(u32::from_le_bytes(raw))
}

fn take_bytes<'a>(
    input: &mut &'a [u8],
) -> Result<&'a [u8], conduit_semantic_catalog::SequenceNormalizationRefusal> {
    let length = usize::try_from(take_u32(input)?)
        .map_err(|_| conduit_semantic_catalog::SequenceNormalizationRefusal::Malformed)?;
    let value = input
        .get(..length)
        .ok_or(conduit_semantic_catalog::SequenceNormalizationRefusal::Malformed)?;
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

pub(super) fn refusal_detail(
    refusal: &conduit_semantic_catalog::SequenceNormalizationRefusal,
) -> u16 {
    use conduit_semantic_catalog::SequenceNormalizationRefusal::*;
    match refusal {
        Malformed => 1,
        Empty => 2,
        TooManyValues => 3,
        ZeroDuration => 4,
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let offer = conduit_std_offers::normalize_sequence_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.configuration.is_empty()
    {
        return Err("planned sequence normalization differs from installed realization".into());
    }
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    budget(placement)?;
    Ok(InstalledOperation::SequenceNormalization(
        SequenceNormalizationOperation {
            pending: false,
            completed: false,
        },
    ))
}
