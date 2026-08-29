//! Ordered finite Boolean delay through the admitted browser timer boundary.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, BROWSER_TIMER_MAXIMUM_MILLIS};
use conduit_core::{
    encode_monotonic_duration, resource_requirement, ConfigurationValue, PlannedGear,
    TIMER_RESOURCE_CLASS,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

const IMPLEMENTATION: &str = "browser/kernel-time-delay-bool@1";
const ARTIFACT: &str = "conduit-browser-runtime/installed-delay@1";

pub(super) static TIME_DELAY: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::time_delay_contract(),
        conduit_semantic_catalog::TIME_DELAY_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![conduit_core::wait_host_operation_requirement()],
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let duration = configured(placement, "duration-ms")?;
    if duration > BROWSER_TIMER_MAXIMUM_MILLIS {
        return Err("time/delay duration-ms exceeds the browser timer bound".into());
    }
    let maximum_values = usize::try_from(configured(placement, "maximum-values")?)
        .map_err(|_| "time/delay maximum-values does not fit")?;
    if maximum_values == 0
        || maximum_values > conduit_semantic_catalog::TIME_MAXIMUM_VALUES as usize
    {
        return Err("time/delay maximum-values exceeds the browser bound".into());
    }
    let durations = (0..maximum_values)
        .map(|_| {
            values
                .store(&encode_monotonic_duration(duration))
                .map_err(debug_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BrowserOperation::installed(DelayOperation {
        durations,
        queued: Vec::with_capacity(maximum_values),
        released: Vec::with_capacity(maximum_values),
        maximum_values,
        accepted: 0,
        next: 0,
        pending: None,
        retained: false,
        closing: false,
        continue_after_emit: false,
    }))
}

fn configured(placement: &PlannedGear, key: &str) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::U64(value)) if found == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("time/delay configuration '{key}' is missing"))
}

struct DelayOperation {
    durations: Vec<ValueRef>,
    queued: Vec<ValueRef>,
    released: Vec<ValueRef>,
    maximum_values: usize,
    accepted: usize,
    next: usize,
    pending: Option<RequestId>,
    retained: bool,
    closing: bool,
    continue_after_emit: bool,
}

impl DelayOperation {
    fn request(&mut self) -> OperationAction {
        let Some(duration) = self.durations.get(self.next).copied() else {
            return fail(40);
        };
        let Ok(raw) = u32::try_from(self.next + 1) else {
            return fail(40);
        };
        let request = RequestId(raw);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(duration, 8).expect("duration is exactly eight bytes"),
        }
    }
}

impl Operation for DelayOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retained = false;
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.closing && self.accepted < self.maximum_values => {
                self.accepted += 1;
                self.retained = true;
                self.queued.push(value);
                if self.pending.is_none() && self.next + 1 == self.queued.len() {
                    self.request()
                } else {
                    OperationAction::Await
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                let Some(value) = self.queued.get(self.next).copied() else {
                    return fail(40);
                };
                self.next += 1;
                self.continue_after_emit = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.closing => {
                self.closing = true;
                if self.pending.is_some() {
                    OperationAction::Await
                } else if self.next < self.queued.len() {
                    self.request()
                } else {
                    OperationAction::Complete
                }
            }
            _ => fail(40),
        }
    }
    fn advance(&mut self) -> OperationAction {
        if !self.continue_after_emit {
            return OperationAction::Await;
        }
        self.continue_after_emit = false;
        if self.next < self.queued.len() {
            self.request()
        } else if self.closing {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }
    fn retains_resumed_value(&self) -> bool {
        self.retained
    }
    fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.pop()
    }
    fn cancel(&mut self) {
        self.pending = None;
        self.queued.clear();
    }
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{HostOperationOutcome, ValueRef};

    fn value(slot: u16, byte_len: u32) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len,
        }
    }

    #[test]
    fn ordered_delay_retains_and_drains_admitted_values() {
        let mut operation = DelayOperation {
            durations: vec![value(10, 8), value(11, 8)],
            queued: Vec::with_capacity(2),
            released: Vec::with_capacity(2),
            maximum_values: 2,
            accepted: 0,
            next: 0,
            pending: None,
            retained: false,
            closing: false,
            continue_after_emit: false,
        };
        let first = value(1, 1);
        let second = value(2, 1);
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: first
            }),
            OperationAction::RequestHostOperation {
                request: RequestId(1),
                ..
            }
        ));
        assert_eq!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: second
            }),
            OperationAction::Await
        );
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Await
        );
        for (request, expected) in [(1, first), (2, second)] {
            assert_eq!(
                operation.resume(OperationInput::HostOperationCompleted {
                    request: RequestId(request),
                    outcome: HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: None,
                        failure: None
                    },
                }),
                OperationAction::Emit {
                    port: PortId(0),
                    value: expected
                }
            );
            if request == 1 {
                assert!(matches!(
                    operation.advance(),
                    OperationAction::RequestHostOperation {
                        request: RequestId(2),
                        ..
                    }
                ));
            }
        }
        assert_eq!(operation.advance(), OperationAction::Complete);
    }
}
