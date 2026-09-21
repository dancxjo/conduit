//! Kernel-side lifecycle for bounded structured rhythm comparison.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::RHYTHM_COMPARE_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct RhythmCompareOperation {
    pending: Option<RequestId>,
    next_request: u32,
    drain_marker: ValueRef,
    release_drain_marker: bool,
    closed: [bool; 2],
    draining_missed: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for RhythmCompareOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 221);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed rhythm comparison completion");
                    io.send(PortId(0), output.value)
                        .expect("ready rhythm comparison output");
                    self.pending = None;
                    if self.draining_missed {
                        let Some((next_request, next)) = self.next_request() else {
                            return step_fail(FailureCode::StorageExhausted, 223);
                        };
                        let input = BoundedValueRef::new(self.drain_marker, 0)
                            .expect("empty rhythm drain marker is exact");
                        io.request_host_call(next_request, HostCallId(0), input)
                            .expect("rhythm comparison drain Host Call");
                        self.next_request = next;
                        self.pending = Some(next_request);
                    }
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed empty rhythm comparison completion");
                    self.pending = None;
                    self.draining_missed = false;
                    if self.closed == [true, true] {
                        io.discard(self.drain_marker)
                            .expect("finished rhythm drain marker");
                        self.release_drain_marker = true;
                        return StepOutcome::Complete;
                    }
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return step_fail(FailureCode::Cancelled, 0)
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    return StepOutcome::Fail(failure)
                }
                _ => return step_fail(FailureCode::InvalidLifecycle, 220),
            }
        }

        for port in [PortId(0), PortId(1)] {
            let Some(value) = io.input(port) else {
                continue;
            };
            if value.byte_len > MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 {
                return step_fail(FailureCode::InvalidInput, 224);
            }
            let index = usize::from(port.0);
            if self.pending.is_some() || self.closed[index] {
                return step_fail(FailureCode::InvalidLifecycle, 221);
            }
            let Some((request, next)) = self.next_request() else {
                return step_fail(FailureCode::StorageExhausted, 223);
            };
            let input = match BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
            {
                Ok(input) => input,
                Err(_) => return step_fail(FailureCode::InvalidInput, 224),
            };
            io.consume(port).expect("present rhythm comparison input");
            io.request_host_call(request, HostCallId(port.0 + 1), input)
                .expect("rhythm comparison Host Call");
            self.next_request = next;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }

        for port in [PortId(0), PortId(1)] {
            let index = usize::from(port.0);
            if io.input_closed(port) && !self.closed[index] {
                io.consume_closed(port)
                    .expect("observed rhythm comparison closure");
                self.closed[index] = true;
                if port == PortId(0) {
                    let Some((request, next)) = self.next_request() else {
                        return step_fail(FailureCode::StorageExhausted, 223);
                    };
                    let input = BoundedValueRef::new(self.drain_marker, 0)
                        .expect("empty rhythm drain marker is exact");
                    io.request_host_call(request, HostCallId(0), input)
                        .expect("rhythm comparison drain Host Call");
                    self.next_request = next;
                    self.pending = Some(request);
                    self.draining_missed = true;
                    return StepOutcome::Progress;
                }
                if self.closed == [true, true] && !self.draining_missed {
                    io.discard(self.drain_marker)
                        .expect("finished rhythm drain marker");
                    self.release_drain_marker = true;
                    return StepOutcome::Complete;
                }
                return StepOutcome::Progress;
            }
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.release_drain_marker = true;
        self.draining_missed = false;
    }
}

impl RhythmCompareOperation {
    fn next_request(&self) -> Option<(RequestId, u32)> {
        self.next_request
            .checked_add(1)
            .map(|next| (RequestId(self.next_request), next))
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

pub(super) fn validate(placement: &PlannedGear) -> Result<(i64, u64), String> {
    let offer = conduit_std_offers::rhythm_compare_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
    {
        return Err("planned rhythm comparison differs from installed realization".into());
    }
    let target = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("target-offset-micros", ConfigurationValue::I64(value)) => Some(*value),
            _ => None,
        })
        .ok_or("rhythm target offset is absent")?;
    let tolerance = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("tolerance-micros", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or("rhythm tolerance is absent")?;
    if !(-60_000_000..=60_000_000).contains(&target) || tolerance > 1_000_000 {
        return Err("rhythm comparison configuration is outside reviewed bounds".into());
    }
    Ok((target, tolerance))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: conduit_semantic_catalog::RHYTHM_MAXIMUM_PENDING_BEATS * 3 + 1,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES
            * usize::from(conduit_semantic_catalog::RHYTHM_MAXIMUM_PENDING_BEATS)
            * 3) as u32,
        host_requests: 3,
        sign_items: conduit_semantic_catalog::RHYTHM_MAXIMUM_PENDING_BEATS.saturating_mul(8),
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let drain_marker = values
        .store(&[])
        .map_err(|error| format!("store rhythm drain marker: {error:?}"))?;
    Ok(InstalledOperation::RhythmCompare(RhythmCompareOperation {
        pending: None,
        next_request: 0,
        drain_marker,
        release_drain_marker: false,
        closed: [false; 2],
        draining_missed: false,
    }))
}

#[cfg(test)]
#[path = "rhythm_compare_operation_tests.rs"]
mod tests;
