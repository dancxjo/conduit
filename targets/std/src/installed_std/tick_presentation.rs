use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDirection};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static TICK_PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TICK_PRESENTATION_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TickPresentationOperation {
    pending: Option<RequestId>,
    next: u32,
}

impl<const PORTS: usize> StepBack<PORTS> for TickPresentationOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return StepOutcome::Fail(tick_failure(FailureCode::InvalidLifecycle));
            }
            let Some(next) = self.next.checked_add(1) else {
                return StepOutcome::Fail(tick_failure(FailureCode::IdentityCapacityExhausted));
            };
            io.consume_host_completion()
                .expect("observed Tick Presentation completion");
            self.pending = None;
            self.next = next;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return StepOutcome::Fail(tick_failure(FailureCode::InvalidLifecycle));
            }
            let Ok(input) = BoundedValueRef::new(value, conduit_time::TICK_ENCODED_LEN) else {
                return StepOutcome::Fail(tick_failure(FailureCode::InvalidLifecycle));
            };
            let request = RequestId(self.next);
            io.consume(PortId(0))
                .expect("present Tick Presentation input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("single Tick Presentation Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed Tick Presentation closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

fn tick_failure(code: FailureCode) -> Failure {
    Failure { code, detail: 9 }
}

impl TickPresentationOperation {}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != conduit_semantic_catalog::TICK_PRESENTATION_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::TICK_PRESENTATION_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::TICK_PRESENTATION_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::TICK_PRESENTATION_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::TICK_PRESENTATION_ARTIFACT
        || placement.inputs.len() != 1
        || !placement.outputs.is_empty()
        || placement.inputs[0].port_id.as_str() != "tick"
        || placement.inputs[0].value_kind.as_str() != conduit_time::TICK_VALUE_KIND
        || placement.inputs[0].direction != PortDirection::Input
        || !placement.configuration.is_empty()
    {
        return Err(
            "planned tick presentation identity does not match its installation".to_string(),
        );
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 1,
        sign_items: 64,
        maximum_value_bytes: conduit_time::TICK_ENCODED_LEN,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::TickPresentation(
        TickPresentationOperation {
            pending: None,
            next: 0,
        },
    ))
}
