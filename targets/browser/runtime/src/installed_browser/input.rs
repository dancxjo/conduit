//! One bounded portable key transition acquired by the browser page adapter.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    kind_id, resource_requirement, HostOperationContractId, HostOperationRequirement, PlannedGear,
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

fn button_offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::button_source_contract(),
        conduit_semantic_catalog::BUTTON_SOURCE_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: BUTTON_IMPLEMENTATION,
            execution_profile: BUTTON_IMPLEMENTATION,
            implementation: BUTTON_IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(BUTTON_EVENT_OPERATION),
            target_kind: Some(kind_id("input/button-transition@1")),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
        }],
        vec![resource_requirement(WINDOW_INPUT_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::keyboard_contract(),
        conduit_semantic_catalog::KEYBOARD_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: KEYBOARD_IMPLEMENTATION,
            execution_profile: KEYBOARD_IMPLEMENTATION,
            implementation: KEYBOARD_IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(KEY_EVENT_OPERATION),
            target_kind: Some(kind_id(conduit_human::KEY_EVENT_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        }],
        vec![resource_requirement(WINDOW_INPUT_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
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
        emitted: false,
    }))
}

fn prepare_button(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &button_offer())?;
    let count = button_transition_count(placement)?;
    let request = (0..count)
        .map(|_| {
            values
                .store(&[0])
                .map_err(|error| format!("store button request: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BrowserOperation::installed(ButtonOperation {
        request,
        pending: false,
        delivered: 0,
    }))
}

fn button_transition_count(placement: &PlannedGear) -> Result<usize, String> {
    match placement.configuration.as_slice() {
        [field] if field.key == "maximum-transitions" => match field.value {
            conduit_core::ConfigurationValue::U64(value)
                if (1..=u64::from(conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_VALUES))
                    .contains(&value) =>
            {
                Ok(value as usize)
            }
            _ => Err("button transition count is outside the admitted 1..8 range".into()),
        },
        _ => Err("button input requires exactly one maximum-transitions configuration".into()),
    }
}

struct ButtonOperation {
    request: Vec<ValueRef>,
    pending: bool,
    delivered: u32,
}

impl ButtonOperation {
    fn request(&mut self) -> OperationAction {
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: RequestId(self.delivered),
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.request[self.delivered as usize], 1)
                .expect("button request is one byte"),
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
                    && request == RequestId(self.delivered)
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
        self.delivered += 1;
        if self.delivered as usize == self.request.len() {
            OperationAction::Complete
        } else {
            self.request()
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

struct KeyboardOperation {
    request: ValueRef,
    pending: bool,
    emitted: bool,
}

impl Operation for KeyboardOperation {
    fn start(&mut self) -> OperationAction {
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.request, 1).expect("keyboard request is one byte"),
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return fail();
                };
                self.pending = false;
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            _ => fail(),
        }
    }
    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            fail()
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{HostOperationOutcome, ValueRef};

    #[test]
    fn button_emits_each_admitted_transition_and_closes_at_the_exact_bound() {
        for count in [1, 2, 5, 8] {
            let mut operation = ButtonOperation {
                request: (0..count)
                    .map(|slot| ValueRef {
                        slot,
                        generation: 1,
                        byte_len: 1,
                    })
                    .collect(),
                pending: false,
                delivered: 0,
            };
            let mut action = operation.start();
            for index in 0..count {
                assert!(matches!(action, OperationAction::RequestHostOperation {
                    request: RequestId(id), ..
                } if id == index as u32));
                let value = operation.request[index as usize];
                assert_eq!(
                    operation.resume(OperationInput::HostOperationCompleted {
                        request: RequestId(index as u32),
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
            assert_eq!(action, OperationAction::Complete);
        }
    }

    #[test]
    fn keyboard_requests_one_bounded_event_and_emits_exact_completion() {
        let mut operation = KeyboardOperation {
            request: ValueRef {
                slot: 1,
                generation: 1,
                byte_len: 1,
            },
            pending: false,
            emitted: false,
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
        assert_eq!(operation.advance(), OperationAction::Complete);
    }
}
