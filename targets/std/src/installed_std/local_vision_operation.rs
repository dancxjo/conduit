//! Installed continuous local-Vision operation lifecycle.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, OperationAction, OperationInput, PortId,
    RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::LOCAL_VISION_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct LocalVisionOperation {
    pending: bool,
    closed: bool,
    next_request: u32,
}

impl<const PORTS: usize> StepOperation<PORTS> for LocalVisionOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.closed {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            let expected = self.next_request.checked_sub(1).map(RequestId);
            if !self.pending
                || expected != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return StepOutcome::Fail(step_failure(332));
            }
            let Some(output) = outcome.output else {
                return StepOutcome::Fail(step_failure(331));
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_host_completion()
                .expect("observed local Vision completion");
            io.send(PortId(0), output.value)
                .expect("ready local Vision output");
            self.pending = false;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending {
                return StepOutcome::Fail(step_failure(332));
            }
            let Ok(input) = BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
            else {
                return StepOutcome::Fail(step_failure(330));
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return StepOutcome::Fail(step_failure(330));
            };
            io.consume(PortId(0)).expect("present local Vision input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("local Vision Host Call");
            self.next_request = next;
            self.pending = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && !self.pending {
            io.consume_closed(PortId(0))
                .expect("observed local Vision closure");
            self.closed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.closed = true;
    }
}

const fn step_failure(detail: u16) -> conduit_kernel::Failure {
    conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    }
}

impl LocalVisionOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.closed => {
                self.pending = true;
                let Ok(input) =
                    BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
                else {
                    return InstalledOperation::fail(330);
                };
                OperationAction::RequestHostCall {
                    request: RequestId(0),
                    operation: HostCallId(0),
                    input,
                }
            }
            OperationInput::HostCallCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending
                && outcome.disposition == HostCallDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return InstalledOperation::fail(331);
                };
                self.pending = false;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                self.closed = true;
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(332),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
        self.closed = true;
    }
}

fn offer(placement: &PlannedGear) -> Result<conduit_core::CapabilityOffer, String> {
    conduit_std_offers::local_vision_offers()
        .into_iter()
        .find(|offer| offer.kind_id == placement.kind_id)
        .ok_or_else(|| "planned local Vision Kind is not installed".to_string())
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer(placement)?;
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || placement.resources.len() != 1
        || placement.resources[0].protected.is_none()
        || placement.authority.len() != 1
        || !placement.configuration.is_empty()
    {
        return Err("planned local Vision operation differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        host_requests: 1,
        sign_items: 32,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::LocalVision(LocalVisionOperation {
        pending: false,
        closed: false,
        next_request: 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{HostCallOutcome, ValueRef};

    fn value() -> ValueRef {
        ValueRef {
            slot: 1,
            generation: 2,
            byte_len: 64,
        }
    }

    #[test]
    fn operation_can_process_repeated_current_images_and_close() {
        let mut operation = LocalVisionOperation {
            pending: false,
            closed: false,
            next_request: 0,
        };
        let request = operation.resume(OperationInput::Value {
            port: PortId(0),
            value: value(),
        });
        assert!(matches!(request, OperationAction::RequestHostCall { .. }));
        assert_eq!(
            operation.resume(OperationInput::HostCallCompleted {
                request: RequestId(0),
                outcome: HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(BoundedValueRef::new(value(), 64).unwrap()),
                    failure: None,
                },
            }),
            OperationAction::Emit {
                port: PortId(0),
                value: value(),
            }
        );
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: value(),
            }),
            OperationAction::RequestHostCall { .. }
        ));
        operation.cancel();
        assert!(operation.closed);
        assert!(!operation.pending);
    }
}
