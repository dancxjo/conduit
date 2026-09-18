//! Optional bounded audible embodiment of one exact frequency quantity.
use super::{
    factory::{validate_placement, BrowserInstallation},
    BrowserOperation,
};
use conduit_core::{kind_id, HostOperationRequirement, PlannedGear, QUANTITY_ENCODED_LEN};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RequestId,
};

pub(crate) const IMPLEMENTATION: &str = "browser/pitch-tone@1";
pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-pitch-tone@1";
pub(crate) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::pitch_tone_contract(),
        conduit_semantic_catalog::PITCH_TONE_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/pitch-tone@1",
        },
        vec![HostOperationRequirement {
            contract_id: HOST_OPERATION.into(),
            target_kind: Some(kind_id("sound/optional-bounded-pitch-tone")),
            maximum_in_flight: 1,
            maximum_input_bytes: QUANTITY_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        vec![conduit_core::resource_requirement(
            super::startup_chime::RESOURCE,
            1,
        )],
        vec![],
    )
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserOperation::installed(PitchTone {
        pending: None,
        next: 0,
    }))
}

struct PitchTone {
    pending: Option<RequestId>,
    next: u32,
}

fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

impl Operation for PitchTone {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let Ok(input) = BoundedValueRef::new(value, QUANTITY_ENCODED_LEN as u32) else {
                    return invalid(1);
                };
                let request = RequestId(self.next);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) && outcome.output.is_none() =>
            {
                let valid = match outcome.disposition {
                    HostOperationDisposition::Completed => outcome.failure.is_none(),
                    HostOperationDisposition::Denied => outcome
                        .failure
                        .is_some_and(|failure| failure.code == FailureCode::HostOperationDenied),
                    HostOperationDisposition::Failed => outcome
                        .failure
                        .is_some_and(|failure| failure.code == FailureCode::HostOperationFailed),
                    HostOperationDisposition::Cancelled => false,
                };
                if !valid {
                    return invalid(2);
                }
                self.pending = None;
                let Some(next) = self.next.checked_add(1) else {
                    return OperationAction::Fail(Failure {
                        code: FailureCode::IdentityCapacityExhausted,
                        detail: 1,
                    });
                };
                self.next = next;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => invalid(3),
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}
