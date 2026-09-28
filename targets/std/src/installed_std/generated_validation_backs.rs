//! Planned kernel edges for generated semantic validation.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static ENVELOPE_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::VALIDATION_ENVELOPE_IMPLEMENTATION,
    budget: envelope_budget,
    prepare: prepare_envelope,
};
pub(super) static VALIDATOR_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::GENERATED_VALIDATOR_IMPLEMENTATION,
    budget: validator_budget,
    prepare: prepare_validator,
};
pub(super) static RETAIN_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::RETAIN_GENERATED_VALIDATION_IMPLEMENTATION,
    budget: retain_budget,
    prepare: prepare_retain,
};

pub(super) struct GeneratedValidationBack {
    first_registered: bool,
    pending: Option<RequestId>,
    complete: bool,
    first_bound: u32,
    second_bound: u32,
    first_call: HostCallId,
    second_call: HostCallId,
}

pub(super) struct GeneratedValidatorBack {
    pending: bool,
    complete: bool,
    input_bound: u32,
}

impl<const PORTS: usize> StepBack<PORTS> for GeneratedValidatorBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(0) {
                return fail(401);
            }
            return finish_output(io, outcome, &mut self.pending, &mut self.complete, 402);
        }
        let Some(value) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        let Ok(input) = BoundedValueRef::new(value, self.input_bound) else {
            return fail(403);
        };
        io.consume(PortId(0)).expect("present validation envelope");
        io.request_host_call(RequestId(0), HostCallId(0), input)
            .expect("semantic validator Host Call");
        self.pending = true;
        StepOutcome::Progress
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.complete = true;
    }
}

impl<const PORTS: usize> StepBack<PORTS> for GeneratedValidationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return fail(404);
            }
            if request == RequestId(0) {
                if outcome.disposition != HostCallDisposition::Completed
                    || outcome.output.is_some()
                    || outcome.failure.is_some()
                {
                    return StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                        code: FailureCode::HostCallFailed,
                        detail: 405,
                    }));
                }
                io.consume_host_completion()
                    .expect("registered validation input");
                self.pending = None;
                self.first_registered = true;
                return StepOutcome::Progress;
            }
            let mut pending = true;
            let result = finish_output(io, outcome, &mut pending, &mut self.complete, 406);
            self.pending = pending.then_some(RequestId(1));
            return result;
        }
        let (port, request, call, bound) = if self.first_registered {
            (PortId(1), RequestId(1), self.second_call, self.second_bound)
        } else {
            (PortId(0), RequestId(0), self.first_call, self.first_bound)
        };
        let Some(value) = io.input(port) else {
            return StepOutcome::Await;
        };
        let Ok(input) = BoundedValueRef::new(value, bound) else {
            return fail(407);
        };
        io.consume(port)
            .expect("present generated validation input");
        io.request_host_call(request, call, input)
            .expect("generated validation Host Call");
        self.pending = Some(request);
        StepOutcome::Progress
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.complete = true;
    }
}

fn finish_output<const PORTS: usize>(
    io: &mut StepIo<PORTS>,
    outcome: conduit_kernel::HostCallOutcome,
    pending: &mut bool,
    complete: &mut bool,
    detail: u16,
) -> StepOutcome {
    if let Some(failure) = outcome.failure {
        return StepOutcome::Fail(failure);
    }
    let Some(output) = outcome.output else {
        return fail(detail);
    };
    if outcome.disposition != HostCallDisposition::Completed || !io.output_ready(PortId(0)) {
        return StepOutcome::Await;
    }
    io.consume_host_completion()
        .expect("observed validation output");
    io.send(PortId(0), output.value)
        .expect("ready validation output");
    *pending = false;
    *complete = true;
    StepOutcome::Complete
}

fn prepare_envelope(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(
        placement,
        conduit_std_offers::VALIDATION_ENVELOPE_IMPLEMENTATION,
    )?;
    Ok(InstalledBack::GeneratedValidationEnvelope(
        GeneratedValidationBack {
            first_registered: false,
            pending: None,
            complete: false,
            first_bound: conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32,
            second_bound: conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
            first_call: HostCallId(1),
            second_call: HostCallId(0),
        },
    ))
}

fn prepare_validator(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(
        placement,
        conduit_std_offers::GENERATED_VALIDATOR_IMPLEMENTATION,
    )?;
    Ok(InstalledBack::GeneratedSemanticValidator(
        GeneratedValidatorBack {
            pending: false,
            complete: false,
            input_bound: conduit_presentation::MAX_GENERATED_VALIDATION_ENVELOPE_BYTES as u32,
        },
    ))
}

fn prepare_retain(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(
        placement,
        conduit_std_offers::RETAIN_GENERATED_VALIDATION_IMPLEMENTATION,
    )?;
    Ok(InstalledBack::RetainGeneratedValidation(
        GeneratedValidationBack {
            first_registered: false,
            pending: None,
            complete: false,
            first_bound: conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
            second_bound: conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
            first_call: HostCallId(0),
            second_call: HostCallId(1),
        },
    ))
}

fn validate(placement: &PlannedGear, implementation: &str) -> Result<(), String> {
    let offer = conduit_std_offers::spoken_mask_offers()
        .into_iter()
        .find(|offer| offer.implementation.implementation_id.as_str() == implementation)
        .ok_or_else(|| "unknown generated validation implementation".to_string())?;
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.host_calls != offer.host_calls
        || !placement.configuration.is_empty()
    {
        return Err("planned generated validation stage differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear, implementation: &str) -> Result<BackBudget, String> {
    validate(placement, implementation)?;
    Ok(BackBudget {
        value_items: 4,
        value_bytes: (conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32)
            .saturating_mul(4),
        host_requests: 2,
        sign_items: 24,
        maximum_value_bytes: conduit_presentation::MAX_GENERATED_VALIDATION_ENVELOPE_BYTES as u32,
    })
}

fn envelope_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    budget(
        placement,
        conduit_std_offers::VALIDATION_ENVELOPE_IMPLEMENTATION,
    )
}
fn validator_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    budget(
        placement,
        conduit_std_offers::GENERATED_VALIDATOR_IMPLEMENTATION,
    )
}
fn retain_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    budget(
        placement,
        conduit_std_offers::RETAIN_GENERATED_VALIDATION_IMPLEMENTATION,
    )
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
