//! Installed bounded ordered-event interval derivation.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::ORDERED_EVENT_INTERVALS_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TimedPatternOperation {
    pending: Option<RequestId>,
    completed: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for TimedPatternOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.completed {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 232);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed timed-pattern completion");
                    io.send(PortId(0), output.value)
                        .expect("ready timed-pattern output");
                    self.pending = None;
                    self.completed = true;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return step_fail(FailureCode::Cancelled, 0)
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    return StepOutcome::Fail(failure)
                }
                _ => return step_fail(FailureCode::InvalidLifecycle, 231),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return step_fail(FailureCode::InvalidLifecycle, 232);
            }
            let Ok(input) = BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
            else {
                return step_fail(FailureCode::InvalidInput, 230);
            };
            io.consume(PortId(0)).expect("present timed-pattern input");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("timed-pattern Host Call");
            self.pending = Some(RequestId(0));
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl TimedPatternOperation {}

pub(super) use conduit_semantic_catalog::BoundedIntervalCodec as TimedPatternHost;

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
        || placement.host_calls != offer.host_calls
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
