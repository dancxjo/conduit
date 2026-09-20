//! One bounded portable key transition acquired by the browser page adapter.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    kind_id, resource_requirement, ArtifactId, CapabilityId, CapabilityOffer,
    CapabilityOfferBuilder, CapabilityRealization, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, PlannedGear,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(crate) const WINDOW_INPUT_RESOURCE_CLASS: &str = "conduit.resource/browser-window-input@1";
pub(crate) const KEY_EVENT_OPERATION: &str = "conduit.host/browser-key-event@1";
pub(crate) const BUTTON_EVENT_OPERATION: &str = "conduit.host/browser-button-transition@1";
pub(super) const KEYBOARD_IMPLEMENTATION: &str = "browser/window-keyboard@1";
pub(super) const BUTTON_IMPLEMENTATION: &str = "browser/window-primary-button@1";
const ARTIFACT: &str = "conduit-browser-runtime/installed-input@1";

pub(super) static KEYBOARD: BrowserInstallation = BrowserInstallation {
    implementation_id: KEYBOARD_IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};
pub(super) static BUTTON: BrowserInstallation = BrowserInstallation {
    implementation_id: BUTTON_IMPLEMENTATION,
    offer: button_offer,
    prepare: prepare_button,
    perform: None,
};

fn button_offer() -> CapabilityOffer {
    CapabilityOfferBuilder::new(
        conduit_semantic_catalog::button_source_semantic_contract(),
        input_realization(
            BUTTON_IMPLEMENTATION,
            BUTTON_EVENT_OPERATION,
            "input/button-transition@1",
            conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
        ),
    )
    .build()
}

fn offer() -> CapabilityOffer {
    CapabilityOfferBuilder::new(
        conduit_semantic_catalog::keyboard_semantic_contract(),
        input_realization(
            KEYBOARD_IMPLEMENTATION,
            KEY_EVENT_OPERATION,
            conduit_human::KEY_EVENT_INFO_ID,
            conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        ),
    )
    .build()
}

fn input_realization(
    implementation: &str,
    operation: &str,
    target: &str,
    maximum_output_bytes: u32,
) -> CapabilityRealization {
    CapabilityRealization {
        capability_id: CapabilityId::from(implementation),
        execution_profile_id: ExecutionProfileId::from(implementation),
        implementation_id: ImplementationId::from(implementation),
        artifact_id: ArtifactId::from(ARTIFACT),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(operation),
            target_kind: Some(kind_id(target)),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes,
        }],
        resource_requirements: vec![resource_requirement(WINDOW_INPUT_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
    }
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let request = values
        .store(&[0])
        .map_err(|error| format!("store keyboard request: {error:?}"))?;
    Ok(BrowserOperation::installed(KeyboardOperation {
        request,
        pending: false,
        next: 0,
    }))
}

fn prepare_button(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &button_offer())?;
    let request = values
        .store(&[0])
        .map_err(|error| format!("store button request: {error:?}"))?;
    Ok(BrowserOperation::installed(ButtonOperation {
        request,
        pending: false,
        next: 0,
    }))
}

struct ButtonOperation {
    request: ValueRef,
    pending: bool,
    next: u32,
}

impl ButtonOperation {
    fn request(&mut self) -> OperationAction {
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: RequestId(self.next),
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.request, 1).expect("button request is one byte"),
        }
    }
}

impl Operation for ButtonOperation {
    fn start(&mut self) -> OperationAction {
        self.request()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending
                    && request == RequestId(self.next)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return fail();
                };
                self.pending = false;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            _ => fail(),
        }
    }

    fn advance(&mut self) -> OperationAction {
        let Some(next) = self.next.checked_add(1) else {
            return identity_exhausted();
        };
        self.next = next;
        self.request()
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

struct KeyboardOperation {
    request: ValueRef,
    pending: bool,
    next: u32,
}

impl Operation for KeyboardOperation {
    fn start(&mut self) -> OperationAction {
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: RequestId(self.next),
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.request, 1).expect("keyboard request is one byte"),
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending
                    && request == RequestId(self.next)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return fail();
                };
                self.pending = false;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            _ => fail(),
        }
    }
    fn advance(&mut self) -> OperationAction {
        let Some(next) = self.next.checked_add(1) else {
            return identity_exhausted();
        };
        self.next = next;
        self.start()
    }
    fn cancel(&mut self) {
        self.pending = false;
    }
}

fn fail() -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 50,
    })
}

fn identity_exhausted() -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::IdentityCapacityExhausted,
        detail: 50,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{HostOperationOutcome, ValueRef};

    #[test]
    fn browser_input_realizations_preserve_owner_capacity_and_fronts() {
        for (offer, contract) in [
            (
                offer(),
                conduit_semantic_catalog::keyboard_semantic_contract(),
            ),
            (
                button_offer(),
                conduit_semantic_catalog::button_source_semantic_contract(),
            ),
        ] {
            assert_eq!(offer.startup_parameters, contract.startup_parameters);
            assert_eq!(offer.shorthand, contract.shorthand);
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(
                offer.kind_contract_revision,
                contract.kind_contract_revision
            );
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
            assert_eq!(offer.host_operations.len(), 1);
            assert_eq!(offer.resource_requirements.len(), 1);
        }
    }

    #[test]
    fn button_rearms_one_fixed_request_after_each_transition() {
        let request = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 1,
        };
        let mut operation = ButtonOperation {
            request,
            pending: false,
            next: 0,
        };
        let mut action = operation.start();
        for sequence in 0..6 {
            assert!(matches!(
                action,
                OperationAction::RequestHostOperation {
                    request: RequestId(found),
                    ..
                } if found == u32::from(sequence)
            ));
            let value = ValueRef {
                slot: sequence + 1,
                generation: 1,
                byte_len: 1,
            };
            assert_eq!(
                operation.resume(OperationInput::HostOperationCompleted {
                    request: RequestId(sequence.into()),
                    outcome: HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: Some(BoundedValueRef::new(value, 1).unwrap()),
                        failure: None,
                    },
                }),
                OperationAction::Emit {
                    port: PortId(0),
                    value
                }
            );
            action = operation.advance();
        }
        assert!(matches!(
            action,
            OperationAction::RequestHostOperation { .. }
        ));
    }

    #[test]
    fn keyboard_rearms_the_same_bounded_request_after_each_event() {
        let mut operation = KeyboardOperation {
            request: ValueRef {
                slot: 1,
                generation: 1,
                byte_len: 1,
            },
            pending: false,
            next: 0,
        };
        assert!(matches!(
            operation.start(),
            OperationAction::RequestHostOperation {
                request: RequestId(0),
                ..
            }
        ));
        let key = ValueRef {
            slot: 2,
            generation: 1,
            byte_len: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        };
        assert_eq!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(key, conduit_human::KEY_EVENT_ENCODED_LEN as u32)
                            .unwrap()
                    ),
                    failure: None,
                },
            }),
            OperationAction::Emit {
                port: PortId(0),
                value: key
            }
        );
        assert!(matches!(
            operation.advance(),
            OperationAction::RequestHostOperation {
                request: RequestId(1),
                ..
            }
        ));
    }
}
