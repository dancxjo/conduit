//! Stateful semantic selection and terminal drive operation for the capstone.

use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, Operation, OperationAction,
    OperationInput, PortId, RequestId, ValueRef,
};

const OPERATION: HostOperationId = HostOperationId(0);
pub(super) const DRIVE_REQUEST: RequestId = RequestId(1);
const SCALAR_BYTES: u32 = conduit_core::SCALAR_ENCODED_LEN as u32;

#[derive(Clone, Copy)]
pub(super) struct CurrentSelector {
    pub(super) selector: Option<bool>,
    pub(super) candidates: [Option<[u8; conduit_core::SCALAR_ENCODED_LEN]>; 2],
    pub(super) closed: [bool; 3],
}

impl Operation for CurrentSelector {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port } if usize::from(port.0) < 3 => {
                self.closed[usize::from(port.0)] = true;
                if self.closed.into_iter().all(|closed| closed) {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => invalid(5),
        }
    }

    fn resume_value(&mut self, port: PortId, _value: ValueRef, bytes: &[u8]) -> OperationAction {
        match port {
            PortId(0) if !self.closed[0] => {
                let Ok(value) = conduit_core::InfoBool::decode(bytes) else {
                    return invalid(6);
                };
                self.selector = Some(value.get());
            }
            PortId(1) | PortId(2) if !self.closed[usize::from(port.0)] => {
                if conduit_core::Scalar::decode(bytes).is_err() {
                    return invalid(7);
                }
                let index = usize::from(port.0 - 1);
                self.candidates[index] = Some(
                    bytes
                        .try_into()
                        .expect("decoded Scalar has the exact canonical length"),
                );
            }
            _ => return invalid(8),
        }
        let Some(selector) = self.selector else {
            return OperationAction::Await;
        };
        if self.candidates.iter().any(Option::is_none) {
            return OperationAction::Await;
        }
        let value = conduit_kernel::CanonicalValue::new(
            &self.candidates[usize::from(selector)].expect("both candidates are present"),
        )
        .expect("Scalar fits the derived-value bound");
        OperationAction::EmitCanonical {
            port: PortId(0),
            value,
        }
    }

    fn cancel(&mut self) {
        self.selector = None;
        self.candidates = [None; 2];
    }
}

#[derive(Clone, Copy)]
pub(super) struct DriveSink {
    pub(super) linear: Option<ValueRef>,
    pub(super) angular_is_zero: bool,
    pub(super) closed: [bool; 2],
    pub(super) pending: bool,
    pub(super) completed: bool,
    pub(super) retain_resumed: bool,
}

impl Operation for DriveSink {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::HostOperationCompleted {
                request: DRIVE_REQUEST,
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                self.pending = false;
                self.completed = true;
                if self.closed.into_iter().all(|closed| closed) {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            OperationInput::Closed { port } if usize::from(port.0) < 2 => {
                self.closed[usize::from(port.0)] = true;
                if self.completed && self.closed.into_iter().all(|closed| closed) {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            OperationInput::HostOperationCompleted { outcome, .. }
                if self.pending
                    && outcome.disposition == HostOperationDisposition::Failed
                    && outcome.failure.is_some() =>
            {
                self.pending = false;
                OperationAction::Fail(outcome.failure.expect("guarded failure"))
            }
            _ => invalid(9),
        }
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, bytes: &[u8]) -> OperationAction {
        self.retain_resumed = false;
        let Ok(scalar) = conduit_core::Scalar::decode(bytes) else {
            return invalid(10);
        };
        match port {
            PortId(0) if self.linear.is_none() && !self.pending && !self.completed => {
                self.linear = Some(value);
                self.retain_resumed = true;
            }
            PortId(1) if !self.angular_is_zero && scalar.raw_microunits() == 0 => {
                self.angular_is_zero = true;
            }
            _ => return invalid(11),
        }
        if !self.angular_is_zero {
            return OperationAction::Await;
        }
        let Some(linear) = self.linear.take() else {
            return OperationAction::Await;
        };
        self.retain_resumed = false;
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: DRIVE_REQUEST,
            operation: OPERATION,
            input: BoundedValueRef::new(linear, 2 * SCALAR_BYTES)
                .expect("linear input is within the planned drive request bound"),
        }
    }

    fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    fn cancel(&mut self) {
        self.linear = None;
        self.pending = false;
        self.completed = false;
        self.retain_resumed = false;
    }
}

const fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}
