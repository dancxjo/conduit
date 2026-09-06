//! Shared finite Flow-to-final-Value normalized-pattern selection.

use conduit_kernel::{Failure, FailureCode, OperationAction, OperationInput, PortId, ValueRef};

pub struct FinalNormalizedPatternOperation {
    latest: Option<ValueRef>,
    released: Option<ValueRef>,
    accepted: u64,
    maximum: u64,
    retain_resumed: bool,
    complete_after_emit: bool,
}

impl FinalNormalizedPatternOperation {
    pub fn new(maximum: u64) -> Self {
        Self {
            latest: None,
            released: None,
            accepted: 0,
            maximum,
            retain_resumed: false,
            complete_after_emit: false,
        }
    }

    pub fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.accepted < self.maximum => {
                self.accepted += 1;
                self.retain_resumed = true;
                self.released = self.latest.replace(value);
                OperationAction::Await
            }
            OperationInput::Value {
                port: PortId(0), ..
            } => fail(FailureCode::StorageExhausted, 1),
            OperationInput::Closed { port: PortId(0) } => {
                let Some(value) = self.latest.take() else {
                    return fail(FailureCode::InvalidInput, 2);
                };
                self.complete_after_emit = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 3),
        }
    }

    pub fn advance(&mut self) -> OperationAction {
        if self.complete_after_emit {
            self.complete_after_emit = false;
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub fn cancel(&mut self) {
        self.latest = None;
        self.released = None;
    }

    pub fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    pub fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.take()
    }
}

impl conduit_kernel::Operation for FinalNormalizedPatternOperation {
    fn start(&mut self) -> OperationAction {
        Self::start(self)
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        Self::resume(self, input)
    }
    fn advance(&mut self) -> OperationAction {
        Self::advance(self)
    }
    fn cancel(&mut self) {
        Self::cancel(self)
    }
    fn retains_resumed_value(&self) -> bool {
        Self::retains_resumed_value(self)
    }
    fn take_released_value(&mut self) -> Option<ValueRef> {
        Self::take_released_value(self)
    }
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(maximum: u64) -> FinalNormalizedPatternOperation {
        FinalNormalizedPatternOperation {
            latest: None,
            released: None,
            accepted: 0,
            maximum,
            retain_resumed: false,
            complete_after_emit: false,
        }
    }

    #[test]
    fn shared_trait_preserves_replacement_release_and_final_emission() {
        let mut concrete = FinalNormalizedPatternOperation::new(2);
        let operation: &mut dyn conduit_kernel::Operation = &mut concrete;
        let first = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 1,
        };
        let second = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: 1,
        };
        assert_eq!(operation.start(), OperationAction::Await);
        for value in [first, second] {
            assert_eq!(
                operation.resume(OperationInput::Value {
                    port: PortId(0),
                    value
                }),
                OperationAction::Await
            );
            assert!(operation.retains_resumed_value());
            assert_eq!(
                operation.take_released_value(),
                if value == first { None } else { Some(first) }
            );
            assert_eq!(operation.take_released_value(), None);
        }
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Emit {
                port: PortId(0),
                value: second
            }
        );
        assert!(!operation.retains_resumed_value());
        assert_eq!(operation.advance(), OperationAction::Complete);
    }

    #[test]
    fn empty_and_over_bound_flows_fail_distinctly() {
        assert!(matches!(
            operation(1).resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 2
            })
        ));
        let mut operation = operation(0);
        let value = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 1,
        };
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value,
            }),
            OperationAction::Fail(Failure {
                code: FailureCode::StorageExhausted,
                detail: 1
            })
        ));
    }
}
