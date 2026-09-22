use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{PlannedGear, PortDirection, PortTemporal, BOOL_ENCODED_LEN};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) fn present_stdout(output: &mut impl std::io::Write, input: &[u8]) -> Result<(), String> {
    let value = conduit_core::InfoBool::decode(input)
        .map_err(|error| format!("Boolean presentation input is invalid: {error:?}"))?;
    writeln!(output, "bool value={}", value.get()).map_err(|error| error.to_string())
}

impl<const PORTS: usize> StepBack<PORTS> for BoolPresentationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return StepOutcome::Fail(step_failure());
            }
            io.consume_host_completion()
                .expect("observed Boolean Presentation completion");
            self.pending = None;
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.output.is_some() {
                return StepOutcome::Fail(step_failure());
            }
            self.next = self.next.saturating_add(1);
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || u64::from(self.next) >= self.maximum {
                return StepOutcome::Fail(step_failure());
            }
            let Ok(input) = BoundedValueRef::new(value, BOOL_ENCODED_LEN as u32) else {
                return StepOutcome::Fail(step_failure());
            };
            let request = RequestId(self.next);
            io.consume(PortId(0))
                .expect("present Boolean Presentation input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("single Boolean Presentation Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed Boolean Presentation closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

fn step_failure() -> conduit_kernel::Failure {
    conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail: 47,
    }
}

pub(super) static BOOL_PRESENTATION_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::BOOL_PRESENTATION_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct BoolPresentationBack {
    pending: Option<RequestId>,
    next: u32,
    maximum: u64,
}

impl BoolPresentationBack {
    pub(super) fn new(maximum: u64) -> Self {
        Self {
            pending: None,
            next: 0,
            maximum,
        }
    }
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::bool_presentation_offer();
    if placement.kind_id.as_str() != conduit_semantic_catalog::BOOL_PRESENTATION_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::BOOL_PRESENTATION_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::BOOL_PRESENTATION_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::BOOL_PRESENTATION_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::BOOL_PRESENTATION_ARTIFACT
        || placement.inputs != offer.inputs
        || !placement.outputs.is_empty()
        || placement.inputs[0].port_id.as_str() != "value"
        || placement.inputs[0].direction != PortDirection::Input
        || placement.inputs[0].temporal != PortTemporal::Current
        || placement.host_calls != offer.host_calls
        || placement.resources.len() != 1
    {
        return Err("planned Boolean presentation identity does not match its installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: conduit_semantic_catalog::MAX_TOGGLE_VALUES as usize,
        sign_items: 64,
        maximum_value_bytes: BOOL_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::BoolPresentation(BoolPresentationBack {
        pending: None,
        next: 0,
        maximum: conduit_semantic_catalog::MAX_TOGGLE_VALUES,
    }))
}
