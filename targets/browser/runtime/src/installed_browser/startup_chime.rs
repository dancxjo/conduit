//! Optional audible embodiment. Lifecycle eligibility belongs to the upstream Form.
use super::{
    factory::{validate_placement, BrowserInstallation},
    BrowserOperation,
};
use conduit_core::{kind_id, HostCallRequirement, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostedValueStore,
    PortId, RequestId,
};

pub(crate) const IMPLEMENTATION: &str = "browser/startup-chime@1";
pub(crate) const HOST_CALL: &str = "conduit.host/browser-startup-chime@1";
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
        vec![HostCallRequirement {
            contract_id: HOST_CALL.into(),
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
    Ok(BrowserOperation::installed_step(Chime {
        pending: None,
        next: 0,
    }))
}
struct Chime {
    pending: Option<RequestId>,
    next: u32,
}
fn invalid() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 1,
    })
}
impl<const PORTS: usize> StepBack<PORTS> for Chime {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending == Some(request) && outcome.output.is_none() {
                let valid = match outcome.disposition {
                    HostCallDisposition::Completed => outcome.failure.is_none(),
                    HostCallDisposition::Denied => outcome
                        .failure
                        .is_some_and(|failure| failure.code == FailureCode::HostCallDenied),
                    HostCallDisposition::Failed => outcome
                        .failure
                        .is_some_and(|failure| failure.code == FailureCode::HostCallFailed),
                    HostCallDisposition::Cancelled => false,
                };
                if !valid {
                    return invalid();
                }
                let Some(next) = self.next.checked_add(1) else {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::IdentityCapacityExhausted,
                        detail: 1,
                    });
                };
                io.consume_host_completion()
                    .expect("observed startup-chime completion");
                self.pending = None;
                self.next = next;
                return StepOutcome::Progress;
            }
            return invalid();
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_none() {
                let Ok(input) = BoundedValueRef::new(value, conduit_core::BOOL_ENCODED_LEN as u32)
                else {
                    return invalid();
                };
                let request = RequestId(self.next);
                io.consume(PortId(0)).expect("present startup-chime input");
                io.request_host_call(request, HostCallId(0), input)
                    .expect("startup-chime Host Call");
                self.pending = Some(request);
                return StepOutcome::Progress;
            }
            return invalid();
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed startup-chime closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
    fn cancel(&mut self) {
        self.pending = None;
    }
}
