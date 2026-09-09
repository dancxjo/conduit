//! Optional audible embodiment. Lifecycle eligibility belongs to the upstream Form.
use super::{
    factory::{validate_placement, BrowserInstallation},
    BrowserOperation,
};
use conduit_core::{kind_id, HostOperationRequirement, PlannedGear};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RequestId,
};

pub(crate) const IMPLEMENTATION: &str = "browser/startup-chime@1";
pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-startup-chime@1";
pub(crate) const RESOURCE: &str = "conduit.resource/browser-audio-cue-slot@1";
pub(crate) const POOL: &str = "browser/audio-cue";
pub(crate) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};
fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::startup_chime_contract(),
        conduit_semantic_catalog::STARTUP_CHIME_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/startup-chime@1",
        },
        vec![HostOperationRequirement {
            contract_id: HOST_OPERATION.into(),
            target_kind: Some(kind_id("sound/optional-audible-cue")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        vec![conduit_core::resource_requirement(RESOURCE, 1)],
        vec![],
    )
}
fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserOperation::installed(Chime {
        pending: None,
        next: 0,
    }))
}
struct Chime {
    pending: Option<RequestId>,
    next: u32,
}
fn invalid() -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 1,
    })
}
impl Operation for Chime {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let Ok(input) = BoundedValueRef::new(value, conduit_core::BOOL_ENCODED_LEN as u32)
                else {
                    return invalid();
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
                    return invalid();
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
            _ => invalid(),
        }
    }
    fn cancel(&mut self) {
        self.pending = None;
    }
}
