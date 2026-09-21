//! Existing browser pointer offer installed in the ordinary form runner.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallId, Operation, OperationAction, OperationInput,
    PortId, RequestId, ValueRef, ValueStorage,
};

pub(crate) const HOST_CALL: &str = "browser.host/pointer-source@1";
pub(super) static POINTER: BrowserInstallation = BrowserInstallation {
    implementation_id: "browser/form-pointer-source@1",
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    crate::browser_pointer::pointer_source_offer(
        "browser-form-pointer-source@1",
        "browser/form-pointer-source@1",
        "browser/form-pointer-source@1",
        "conduit-browser-runtime/form-pointer-source@1",
        super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        vec![conduit_core::ResourceRequirement {
            class_id: super::input::WINDOW_INPUT_RESOURCE_CLASS.into(),
            units: 1,
            content: None,
            protected_role: None,
            compute: None,
        }],
    )
}

fn prepare(
    placement: &conduit_core::PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let empty = values
        .store(&[])
        .map_err(|error| format!("pointer request: {error:?}"))?;
    Ok(BrowserOperation::installed(PointerSource {
        empty,
        pending: false,
        next: 0,
    }))
}

struct PointerSource {
    empty: ValueRef,
    pending: bool,
    next: u32,
}

impl Operation for PointerSource {
    fn start(&mut self) -> OperationAction {
        if self.pending {
            return fail();
        }
        self.pending = true;
        OperationAction::RequestHostCall {
            request: RequestId(self.next),
            operation: HostCallId(0),
            input: BoundedValueRef::new(self.empty, 0).expect("empty pointer request"),
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        if let OperationInput::HostCallCompleted { request, outcome } = input {
            if !self.pending || request != RequestId(self.next) {
                return fail();
            }
            self.pending = false;
            return match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None)
                    if output.admitted_bytes == super::MAXIMUM_BROWSER_VALUE_BYTES as u32 =>
                {
                    OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    }
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    OperationAction::Fail(failure)
                }
                _ => fail(),
            };
        }
        fail()
    }
    fn advance(&mut self) -> OperationAction {
        let Some(next) = self.next.checked_add(1) else {
            return OperationAction::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::IdentityCapacityExhausted,
                detail: 21,
            });
        };
        self.next = next;
        self.start()
    }
    fn cancel(&mut self) {
        self.pending = false;
    }
}

fn fail() -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidInput,
        detail: 21,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::HostCallOutcome;

    #[test]
    fn pointer_rearms_after_multiple_separated_observations() {
        let mut operation = PointerSource {
            empty: ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 0,
            },
            pending: false,
            next: 0,
        };
        let mut action = operation.start();
        for slot in 1..=3 {
            assert!(matches!(action, OperationAction::RequestHostCall { .. }));
            let value = ValueRef {
                slot,
                generation: 1,
                byte_len: 16,
            };
            assert_eq!(
                operation.resume(OperationInput::HostCallCompleted {
                    request: RequestId((slot - 1).into()),
                    outcome: HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(
                                value,
                                super::super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
                            )
                            .unwrap(),
                        ),
                        failure: None,
                    },
                }),
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            );
            action = operation.advance();
        }
        assert!(matches!(action, OperationAction::RequestHostCall { .. }));
    }
}
