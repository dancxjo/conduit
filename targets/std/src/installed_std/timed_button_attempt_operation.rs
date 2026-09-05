//! Installed lifecycle for one finite pressed-button timing attempt.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{encode_monotonic_duration, ConfigurationValue, PlannedGear};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, OperationAction, OperationInput, PortId, RequestId, ValueRef,
    ValueStorage,
};
use std::vec::Vec;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TIMED_BUTTON_ATTEMPT_STD_IMPLEMENTATION,
    budget,
    prepare,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Observe,
    Deadline,
}

pub(super) struct TimedButtonAttemptOperation {
    durations: Vec<ValueRef>,
    released: Vec<ValueRef>,
    next_duration: usize,
    next_request: u32,
    pending: Option<(RequestId, Pending)>,
    cancellation: Option<RequestId>,
    queued_transition: Option<ValueRef>,
    accepted_transitions: u64,
    maximum_transitions: u64,
    retain_resumed: bool,
    completed: bool,
}

impl TimedButtonAttemptOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.completed && self.accepted_transitions < self.maximum_transitions => {
                self.accepted_transitions += 1;
                match self.pending {
                    None => self.request_observation(value),
                    Some((request, Pending::Deadline)) if self.queued_transition.is_none() => {
                        self.retain_resumed = true;
                        self.queued_transition = Some(value);
                        self.cancellation = Some(request);
                        OperationAction::Await
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 271),
                }
            }
            OperationInput::Value {
                port: PortId(0), ..
            } if self.accepted_transitions >= self.maximum_transitions => {
                fail(FailureCode::StorageExhausted, 1)
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some((request, Pending::Deadline)) =>
            {
                self.resume_deadline(request, outcome)
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                self.release_unused_durations();
                fail(FailureCode::InvalidInput, 2)
            }
            _ => fail(FailureCode::InvalidLifecycle, 272),
        }
    }

    pub(super) fn resume_host_operation(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        if self.pending != Some((request, Pending::Observe)) {
            return self.resume(OperationInput::HostOperationCompleted { request, outcome });
        }
        self.pending = None;
        match (
            outcome.disposition,
            outcome.output,
            outcome.failure,
            canonical,
        ) {
            (HostOperationDisposition::Completed, None, None, None) => OperationAction::Await,
            (HostOperationDisposition::Completed, Some(_), None, Some([0])) => {
                self.request_deadline()
            }
            (HostOperationDisposition::Completed, Some(output), None, Some(_)) => {
                self.completed = true;
                self.release_unused_durations();
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            (HostOperationDisposition::Cancelled, _, _, _) => fail(FailureCode::Cancelled, 0),
            (HostOperationDisposition::Failed, None, Some(failure), _) => {
                OperationAction::Fail(failure)
            }
            _ => fail(FailureCode::InvalidLifecycle, 273),
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
        self.cancellation = None;
        self.queued_transition = None;
    }

    pub(super) fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.pop()
    }

    pub(super) fn take_host_operation_cancellation(&mut self) -> Option<RequestId> {
        self.cancellation.take()
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        self.durations.capacity() + self.released.capacity()
    }

    fn resume_deadline(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
    ) -> OperationAction {
        self.pending = None;
        match (outcome.disposition, outcome.output, outcome.failure) {
            (HostOperationDisposition::Cancelled, None, None) => {
                self.queued_transition.take().map_or_else(
                    || fail(FailureCode::InvalidLifecycle, 274),
                    |value| self.request_observation(value),
                )
            }
            (HostOperationDisposition::Completed, None, None) => {
                self.release_unused_durations();
                fail(FailureCode::HostOperationFailed, 4)
            }
            (HostOperationDisposition::Failed, None, Some(failure)) => {
                OperationAction::Fail(failure)
            }
            _ => {
                let _ = request;
                fail(FailureCode::InvalidLifecycle, 275)
            }
        }
    }

    fn request_observation(&mut self, value: ValueRef) -> OperationAction {
        let request = self.next_request();
        self.pending = Some((request, Pending::Observe));
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(1),
            input: BoundedValueRef::new(
                value,
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            )
            .expect("button transition is bounded by its exact port"),
        }
    }

    fn request_deadline(&mut self) -> OperationAction {
        let Some(value) = self.durations.get(self.next_duration).copied() else {
            return fail(FailureCode::StorageExhausted, 276);
        };
        self.next_duration += 1;
        let request = self.next_request();
        self.pending = Some((request, Pending::Deadline));
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(value, 8)
                .expect("deadline duration is exactly eight bytes"),
        }
    }

    fn next_request(&mut self) -> RequestId {
        let request = RequestId(self.next_request);
        self.next_request = self.next_request.saturating_add(1);
        request
    }

    fn release_unused_durations(&mut self) {
        self.released
            .extend(self.durations.drain(self.next_duration..));
    }
}

fn configuration(placement: &PlannedGear, key: &str) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, ConfigurationValue::U64(value)) if candidate == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("pressed-button attempt lacks {key}"))
}

fn validate(placement: &PlannedGear) -> Result<(u64, u64, u64), String> {
    let offer = conduit_std_offers::timed_button_attempt_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
        || !placement.authority.is_empty()
    {
        return Err("planned pressed-button attempt differs from installed realization".into());
    }
    for class in [
        conduit_core::TIMER_RESOURCE_CLASS,
        conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS,
    ] {
        if !placement.resources.iter().any(|resource| {
            resource.class_id.as_str() == class
                && resource.units == 1
                && resource.protected.is_none()
                && resource.compute.is_none()
        }) {
            return Err(format!("pressed-button attempt lacks admitted {class}"));
        }
    }
    let presses = configuration(placement, "maximum-presses")?;
    let transitions = configuration(placement, "maximum-transitions")?;
    let timeout = configuration(placement, "timeout-ms")?;
    if !(2..=conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS as u64).contains(&presses)
        || !(presses..=conduit_semantic_catalog::MAXIMUM_ATTEMPT_TRANSITIONS).contains(&transitions)
        || !(1..=conduit_semantic_catalog::MAXIMUM_ATTEMPT_TIMEOUT_MS).contains(&timeout)
    {
        return Err("pressed-button attempt configuration is outside reviewed bounds".into());
    }
    Ok((presses, transitions, timeout))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let (_, transitions, _) = validate(placement)?;
    Ok(OperationBudget {
        value_items: u16::try_from(transitions.saturating_mul(3) + 1)
            .map_err(|_| "pressed-button value budget overflow")?,
        value_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
            .saturating_mul(transitions as u32 + 1),
        host_requests: usize::try_from(transitions.saturating_mul(2))
            .map_err(|_| "pressed-button request budget overflow")?,
        sign_items: u16::try_from(transitions.saturating_mul(12))
            .map_err(|_| "pressed-button sign budget overflow")?,
        maximum_value_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    let (_, transitions, timeout) = validate(placement)?;
    let durations = (0..transitions)
        .map(|_| {
            values
                .store(&encode_monotonic_duration(timeout))
                .map_err(|error| format!("store pressed-button deadline: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InstalledOperation::TimedButtonAttempt(
        TimedButtonAttemptOperation {
            durations,
            released: Vec::with_capacity(transitions as usize + 1),
            next_duration: 0,
            next_request: 0,
            pending: None,
            cancellation: None,
            queued_transition: None,
            accepted_transitions: 0,
            maximum_transitions: transitions,
            retain_resumed: false,
            completed: false,
        },
    ))
}

pub(super) fn host_maximum(placement: &PlannedGear) -> Result<usize, String> {
    usize::try_from(validate(placement)?.0)
        .map_err(|_| "pressed-button maximum does not fit this Host".into())
}

pub(super) fn refusal_detail(refusal: super::timed_button_attempt_host::Refusal) -> u16 {
    use super::timed_button_attempt_host::Refusal::*;
    match refusal {
        MalformedTransition => 1,
        TooManyEvents => 2,
        ClockRegressed => 3,
    }
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(
        store: &mut conduit_kernel::HostedValueStore,
        maximum_transitions: u64,
    ) -> TimedButtonAttemptOperation {
        let durations = (0..maximum_transitions)
            .map(|_| store.store(&encode_monotonic_duration(50)).unwrap())
            .collect();
        TimedButtonAttemptOperation {
            durations,
            released: Vec::with_capacity(maximum_transitions as usize + 1),
            next_duration: 0,
            next_request: 0,
            pending: None,
            cancellation: None,
            queued_transition: None,
            accepted_transitions: 0,
            maximum_transitions,
            retain_resumed: false,
            completed: false,
        }
    }

    #[test]
    fn fired_deadline_is_a_distinct_timeout_failure() {
        let mut store = conduit_kernel::HostedValueStore::new(8, 1024, 4096).unwrap();
        let transition = store.store(b"transition").unwrap();
        let marker = store.store(&[0]).unwrap();
        let mut operation = operation(&mut store, 2);
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: transition
            }),
            OperationAction::RequestHostOperation {
                operation: HostOperationId(1),
                ..
            }
        ));
        assert!(matches!(
            operation.resume_host_operation(
                RequestId(0),
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(BoundedValueRef::new(marker, 1).unwrap()),
                    failure: None,
                },
                Some(&[0]),
            ),
            OperationAction::RequestHostOperation {
                operation: HostOperationId(0),
                ..
            }
        ));
        assert!(matches!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(1),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                }
            }),
            OperationAction::Fail(Failure {
                code: FailureCode::HostOperationFailed,
                detail: 4
            })
        ));
    }

    #[test]
    fn total_transition_exhaustion_is_not_timeout_or_malformed_input() {
        let mut store = conduit_kernel::HostedValueStore::new(8, 1024, 4096).unwrap();
        let first = store.store(b"released-1").unwrap();
        let second = store.store(b"released-2").unwrap();
        let mut operation = operation(&mut store, 1);
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: first
            }),
            OperationAction::RequestHostOperation { .. }
        ));
        assert!(matches!(
            operation.resume_host_operation(
                RequestId(0),
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
                None,
            ),
            OperationAction::Await
        ));
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: second
            }),
            OperationAction::Fail(Failure {
                code: FailureCode::StorageExhausted,
                detail: 1
            })
        ));
    }
}
