//! Optional bounded audible embodiment of one exact frequency quantity.
use super::{
    factory::{validate_placement, BrowserInstallation},
    BrowserOperation,
};
use conduit_core::{kind_id, HostCallRequirement, PlannedGear, QUANTITY_ENCODED_LEN};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostedValueStore,
    PortId, RequestId,
};

pub(crate) const IMPLEMENTATION: &str = "browser/pitch-tone@1";
pub(crate) const HOST_CALL: &str = "conduit.host/browser-pitch-tone@1";
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
        vec![HostCallRequirement {
            contract_id: HOST_CALL.into(),
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
    Ok(BrowserOperation::installed_step(PitchTone {
        pending: None,
        next: 0,
    }))
}

struct PitchTone {
    pending: Option<RequestId>,
    next: u32,
}

fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

impl<const PORTS: usize> StepOperation<PORTS> for PitchTone {
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
                    return invalid(2);
                }
                let Some(next) = self.next.checked_add(1) else {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::IdentityCapacityExhausted,
                        detail: 1,
                    });
                };
                io.consume_host_completion()
                    .expect("observed pitch-tone completion");
                self.pending = None;
                self.next = next;
                return StepOutcome::Progress;
            }
            return invalid(3);
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_none() {
                let Ok(input) = BoundedValueRef::new(value, QUANTITY_ENCODED_LEN as u32) else {
                    return invalid(1);
                };
                let request = RequestId(self.next);
                io.consume(PortId(0)).expect("present pitch-tone input");
                io.request_host_call(request, HostCallId(0), input)
                    .expect("pitch-tone Host Call");
                self.pending = Some(request);
                return StepOutcome::Progress;
            }
            return invalid(3);
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed pitch-tone closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}
