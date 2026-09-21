//! One bounded portable key transition acquired by the browser page adapter.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    kind_id, resource_requirement, HostCallContractId, HostCallRequirement, PlannedGear,
};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
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
    let mut offer = conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::button_source_contract(),
        conduit_semantic_catalog::BUTTON_SOURCE_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: BUTTON_IMPLEMENTATION,
            execution_profile: BUTTON_IMPLEMENTATION,
            implementation: BUTTON_IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostCallRequirement {
            contract_id: HostCallContractId::from(BUTTON_EVENT_OPERATION),
            target_kind: Some(kind_id("input/button-transition@1")),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
        }],
        vec![resource_requirement(WINDOW_INPUT_RESOURCE_CLASS, 1)],
        Vec::new(),
    );
    offer.limits.max_active_instances = super::MAXIMUM_BROWSER_GEARS as u16;
    offer
}

fn offer() -> conduit_core::CapabilityOffer {
    let mut offer = conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::keyboard_contract(),
        conduit_semantic_catalog::KEYBOARD_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: KEYBOARD_IMPLEMENTATION,
            execution_profile: KEYBOARD_IMPLEMENTATION,
            implementation: KEYBOARD_IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostCallRequirement {
            contract_id: HostCallContractId::from(KEY_EVENT_OPERATION),
            target_kind: Some(kind_id(conduit_human::KEY_EVENT_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        }],
        vec![resource_requirement(WINDOW_INPUT_RESOURCE_CLASS, 1)],
        Vec::new(),
    );
    offer.limits.max_active_instances = super::MAXIMUM_BROWSER_GEARS as u16;
    offer
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let request = values
        .store(&[0])
        .map_err(|error| format!("store keyboard request: {error:?}"))?;
    Ok(BrowserOperation::installed_step(KeyboardOperation {
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
    Ok(BrowserOperation::installed_step(ButtonOperation {
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

struct KeyboardOperation {
    request: ValueRef,
    pending: bool,
    next: u32,
}

fn continuous_input_step<const PORTS: usize>(
    request_value: ValueRef,
    pending: &mut bool,
    next: &mut u32,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if let Some((request, outcome)) = io.host_completion() {
        if !*pending
            || request != RequestId(*next)
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.failure.is_some()
        {
            return fail();
        }
        let Some(output) = outcome.output else {
            return fail();
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(following) = next.checked_add(1) else {
            return identity_exhausted();
        };
        io.consume_host_completion()
            .expect("observed browser input completion");
        io.send(PortId(0), output.value)
            .expect("ready browser input output");
        *pending = false;
        *next = following;
    }
    if !*pending {
        io.request_host_call(
            RequestId(*next),
            HostCallId(0),
            BoundedValueRef::new(request_value, 1).expect("browser input request is one byte"),
        )
        .expect("browser input Host Call");
        *pending = true;
        return StepOutcome::Progress;
    }
    StepOutcome::Await
}

impl<const PORTS: usize> StepOperation<PORTS> for ButtonOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        continuous_input_step(self.request, &mut self.pending, &mut self.next, io)
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

impl<const PORTS: usize> StepOperation<PORTS> for KeyboardOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        continuous_input_step(self.request, &mut self.pending, &mut self.next, io)
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

fn fail() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 50,
    })
}

fn identity_exhausted() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::IdentityCapacityExhausted,
        detail: 50,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{HostCallOutcome, ValueRef};

    fn initial<O: StepOperation<1>>(operation: &mut O) -> StepIo<1> {
        let mut io = StepIo::test_frame([None], [false], [Some(4096)], None, 4);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        io
    }

    fn complete<O: StepOperation<1>>(
        operation: &mut O,
        request: RequestId,
        output: BoundedValueRef,
    ) -> StepIo<1> {
        let mut io = StepIo::test_frame(
            [None],
            [false],
            [Some(4096)],
            Some((
                request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            )),
            4,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        io
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
        let mut io = initial(&mut operation);
        for sequence in 0..6 {
            assert_eq!(
                io.test_host_request().map(|request| request.0),
                Some(RequestId(sequence.into()))
            );
            let value = ValueRef {
                slot: sequence + 1,
                generation: 1,
                byte_len: 1,
            };
            io = complete(
                &mut operation,
                RequestId(sequence.into()),
                BoundedValueRef::new(value, 1).unwrap(),
            );
            assert_eq!(io.test_output(PortId(0)), Some(value));
        }
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(6))
        );
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
        let io = initial(&mut operation);
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(0))
        );
        let key = ValueRef {
            slot: 2,
            generation: 1,
            byte_len: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        };
        let io = complete(
            &mut operation,
            RequestId(0),
            BoundedValueRef::new(key, conduit_human::KEY_EVENT_ENCODED_LEN as u32).unwrap(),
        );
        assert_eq!(io.test_output(PortId(0)), Some(key));
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(1))
        );
    }
}
