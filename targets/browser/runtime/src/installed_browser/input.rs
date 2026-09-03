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
pub(super) const KEYBOARD_IMPLEMENTATION: &str = "browser/window-keyboard@1";
const ARTIFACT: &str = "conduit-browser-runtime/installed-input@1";

pub(super) static KEYBOARD: BrowserInstallation = BrowserInstallation {
    implementation_id: KEYBOARD_IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

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
