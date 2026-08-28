//! Kernel-side lifecycle for bounded structured rhythm comparison.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
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

impl RhythmCompareOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { value, .. }
                if value.byte_len > MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 =>
            {
                fail(FailureCode::InvalidInput, 224)
            }
            OperationInput::Value { port, value }
                if self.pending.is_none()
                    && matches!(port, PortId(0) | PortId(1))
                    && !self.closed[usize::from(port.0)] =>
            {
                self.request(HostOperationId(port.0 + 1), value)
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Completed, None, None) => {
                        if self.draining_missed {
                            self.draining_missed = false;
                        }
                        self.complete_or_await()
                    }
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 0),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 220),
                }
            }
            OperationInput::Closed {
                port: PortId(port @ 0..=1),
            } if self.pending.is_none() && !self.closed[usize::from(port)] => {
                self.closed[usize::from(port)] = true;
                if port == 0 {
                    self.draining_missed = true;
                    self.request(HostOperationId(0), self.drain_marker)
                } else {
                    self.complete_or_await()
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 221),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.draining_missed {
            self.request(HostOperationId(0), self.drain_marker)
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.release_drain_marker = true;
        self.draining_missed = false;
    }

    pub(super) fn retains_resumed_value(&self) -> bool {
        false
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.release_drain_marker.then(|| {
            self.release_drain_marker = false;
            self.drain_marker
        })
    }

    fn request(&mut self, operation: HostOperationId, value: ValueRef) -> OperationAction {
        let request = RequestId(self.next_request);
        let Some(next) = self.next_request.checked_add(1) else {
            return fail(FailureCode::StorageExhausted, 223);
        };
        self.next_request = next;
        self.pending = Some(request);
        let maximum = if operation == HostOperationId(0) {
            0
        } else {
            MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
        };
        let Ok(input) = BoundedValueRef::new(value, maximum) else {
            self.pending = None;
            return fail(FailureCode::InvalidInput, 224);
        };
        OperationAction::RequestHostOperation {
            request,
            operation,
            input,
        }
    }

    fn complete_or_await(&mut self) -> OperationAction {
        if self.closed == [true, true] {
            self.release_drain_marker = true;
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }
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
        || placement.host_operations != offer.host_operations
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

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
#[path = "rhythm_compare_operation_tests.rs"]
mod tests;
