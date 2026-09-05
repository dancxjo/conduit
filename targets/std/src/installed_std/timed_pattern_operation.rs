//! Installed bounded ordered-event interval derivation.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};
use std::vec::Vec;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::ORDERED_EVENT_INTERVALS_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TimedPatternOperation {
    pending: Option<RequestId>,
    completed: bool,
}

impl TimedPatternOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && !self.completed => {
                self.pending = Some(RequestId(0));
                let Ok(input) =
                    BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
                else {
                    return InstalledOperation::fail(230);
                };
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
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
                    _ => InstalledOperation::fail(231),
                }
            }
            OperationInput::Closed { port: PortId(0) }
                if self.pending.is_none() && self.completed =>
            {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(232),
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
        self.pending = None;
    }
}

pub(super) struct TimedPatternHost {
    input_type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    output: Vec<u8>,
}

impl TimedPatternHost {
    pub(super) fn prepare() -> Self {
        Self {
            input_type_prefix: conduit_semantic_catalog::timed_event_sequence_type()
                .canonical_bytes()
                .expect("installed timed-event type is canonical"),
            output_type_prefix: conduit_semantic_catalog::interval_sequence_type()
                .canonical_bytes()
                .expect("installed interval type is canonical"),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    pub(super) fn execute(
        &mut self,
        input: &[u8],
    ) -> Result<&[u8], conduit_semantic_catalog::TimedPatternRefusal> {
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
) -> Result<(&'a [u8], &'a [u8]), conduit_semantic_catalog::TimedPatternRefusal> {
    let mut input = input
        .strip_prefix(type_prefix)
        .ok_or(conduit_semantic_catalog::TimedPatternRefusal::Malformed)?;
    if take_byte(&mut input)? != 2 || take_u32(&mut input)? != 2 {
        return Err(conduit_semantic_catalog::TimedPatternRefusal::Malformed);
    }
    let clock_basis = take_named_leaf(&mut input, "clock_basis")?;
    let event_times = take_named_leaf(&mut input, "event_times")?;
    if clock_basis.is_empty() || !input.is_empty() {
        return Err(conduit_semantic_catalog::TimedPatternRefusal::Malformed);
    }
    core::str::from_utf8(clock_basis)
        .map_err(|_| conduit_semantic_catalog::TimedPatternRefusal::Malformed)?;
    Ok((clock_basis, event_times))
}

fn encode_output(
    output: &mut Vec<u8>,
    type_prefix: &[u8],
    clock_basis: &[u8],
    event_times: &[u8],
) -> Result<(), conduit_semantic_catalog::TimedPatternRefusal> {
    use conduit_semantic_catalog::TimedPatternRefusal;

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
        if count > conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS {
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

fn parse_u64(raw: &[u8]) -> Result<u64, conduit_semantic_catalog::TimedPatternRefusal> {
    let mut value = 0_u64;
    for digit in raw {
        if !digit.is_ascii_digit() {
            return Err(conduit_semantic_catalog::TimedPatternRefusal::Malformed);
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(*digit - b'0')))
            .ok_or(conduit_semantic_catalog::TimedPatternRefusal::Malformed)?;
    }
    Ok(value)
}

fn take_named_leaf<'a>(
    input: &mut &'a [u8],
    name: &str,
) -> Result<&'a [u8], conduit_semantic_catalog::TimedPatternRefusal> {
    if take_bytes(input)? != name.as_bytes() || take_byte(input)? != 0 {
        return Err(conduit_semantic_catalog::TimedPatternRefusal::Malformed);
    }
    take_bytes(input)
}

fn take_byte(input: &mut &[u8]) -> Result<u8, conduit_semantic_catalog::TimedPatternRefusal> {
    let (&value, rest) = input
        .split_first()
        .ok_or(conduit_semantic_catalog::TimedPatternRefusal::Malformed)?;
    *input = rest;
    Ok(value)
}

fn take_u32(input: &mut &[u8]) -> Result<u32, conduit_semantic_catalog::TimedPatternRefusal> {
    let raw: [u8; 4] = input
        .get(..4)
        .ok_or(conduit_semantic_catalog::TimedPatternRefusal::Malformed)?
        .try_into()
        .map_err(|_| conduit_semantic_catalog::TimedPatternRefusal::Malformed)?;
    *input = &input[4..];
    Ok(u32::from_le_bytes(raw))
}

fn take_bytes<'a>(
    input: &mut &'a [u8],
) -> Result<&'a [u8], conduit_semantic_catalog::TimedPatternRefusal> {
    let length = usize::try_from(take_u32(input)?)
        .map_err(|_| conduit_semantic_catalog::TimedPatternRefusal::Malformed)?;
    let value = input
        .get(..length)
        .ok_or(conduit_semantic_catalog::TimedPatternRefusal::Malformed)?;
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

pub(super) fn refusal_detail(refusal: &conduit_semantic_catalog::TimedPatternRefusal) -> u16 {
    use conduit_semantic_catalog::TimedPatternRefusal::*;
    match refusal {
        Malformed => 1,
        TooFewEvents => 2,
        TooManyEvents => 3,
        ReorderedOrDuplicateEvent => 4,
        IntervalOverflow => 5,
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let offer = conduit_std_offers::ordered_event_intervals_std_offer();
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
        return Err("planned ordered-event intervals differ from installed realization".into());
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
    Ok(InstalledOperation::TimedPattern(TimedPatternOperation {
        pending: None,
        completed: false,
    }))
}
