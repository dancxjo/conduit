//! Installed bounded normalized-pattern comparison.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

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

pub(super) struct PatternComparisonHost(conduit_semantic_catalog::BoundedPatternComparisonCodec);

impl PatternComparisonHost {
    pub(super) fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        conduit_semantic_catalog::BoundedPatternComparisonCodec::new(validate(placement)?).map(Self)
    }

    pub(super) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<Option<&[u8]>, conduit_semantic_catalog::PatternComparisonRefusal> {
        use conduit_semantic_catalog::PatternComparisonInput;
        let port = match contract {
            conduit_std_offers::COMPARE_PATTERN_CANDIDATE_OPERATION => {
                PatternComparisonInput::Candidate
            }
            conduit_std_offers::COMPARE_PATTERN_TEMPLATE_OPERATION => {
                PatternComparisonInput::Template
            }
            _ => return Err(conduit_semantic_catalog::PatternComparisonRefusal::Malformed),
        };
        self.0.execute(port, input)
    }
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
