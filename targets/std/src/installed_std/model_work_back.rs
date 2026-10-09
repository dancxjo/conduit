//! Bounded, sequential requests through the production Kernel HostCall boundary.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) const MAXIMUM_REQUESTS: u16 = 256;
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_ai::MODEL_WORK_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct ModelWorkBack {
    completed: u16,
    pending: bool,
}
impl<const PORTS: usize> StepBack<PORTS> for ModelWorkBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(u32::from(self.completed)) {
                return fail(FailureCode::InvalidLifecycle, 86);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed model-work completion");
                    io.send(PortId(0), output.value)
                        .expect("ready model-work reply");
                    self.pending = false;
                    self.completed += 1;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Denied, _, _) => fail(FailureCode::HostCallDenied, 82),
                (HostCallDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 83),
                (HostCallDisposition::Failed, _, _) => {
                    StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                        code: FailureCode::HostCallFailed,
                        detail: 84,
                    }))
                }
                _ => fail(FailureCode::InvalidLifecycle, 85),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending {
                return fail(FailureCode::InvalidLifecycle, 86);
            }
            if self.completed >= MAXIMUM_REQUESTS {
                return fail(FailureCode::InvalidInput, 87);
            }
            let Ok(input) = BoundedValueRef::new(value, conduit_ai::MODEL_WORK_MAXIMUM_INPUT_BYTES)
            else {
                return fail(FailureCode::InvalidInput, 81);
            };
            io.consume(PortId(0)).expect("present model-work command");
            io.request_host_call(RequestId(u32::from(self.completed)), HostCallId(0), input)
                .expect("model-work HostCall");
            self.pending = true;
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) && !self.pending {
            io.consume_closed(PortId(0))
                .expect("observed model-work closure");
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
    fn cancel(&mut self) {
        self.pending = false;
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
pub(super) fn validate(placement: &PlannedGear) -> Result<(), String> {
    let kind = conduit_ai::model_work_kind();
    if placement.kind_id != kind.kind_id
        || placement.kind_contract_revision != kind.kind_contract_revision
        || placement.implementation_id.as_str() != conduit_ai::MODEL_WORK_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_ai::MODEL_WORK_ARTIFACT
        || placement.execution_profile_id.as_str() != conduit_ai::MODEL_WORK_PROFILE
        || placement.inputs != kind.inputs
        || placement.outputs != kind.outputs
        || !placement.configuration.is_empty()
        || placement.host_calls.len() != 1
        || placement.host_calls[0].contract_id.as_str() != conduit_ai::MODEL_WORK_OPERATION
        || placement.host_calls[0].maximum_in_flight != 1
        || placement.host_calls[0].maximum_input_bytes != conduit_ai::MODEL_WORK_MAXIMUM_INPUT_BYTES
        || placement.host_calls[0].maximum_output_bytes
            != conduit_ai::MODEL_WORK_MAXIMUM_OUTPUT_BYTES
    {
        return Err("planned model-work contract does not match its installation".into());
    }
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: MAXIMUM_REQUESTS,
        value_bytes: conduit_ai::MODEL_WORK_MAXIMUM_OUTPUT_BYTES
            .checked_mul(u32::from(MAXIMUM_REQUESTS))
            .ok_or("model-work value budget overflow")?,
        host_requests: usize::from(MAXIMUM_REQUESTS),
        sign_items: MAXIMUM_REQUESTS * 32,
        maximum_value_bytes: conduit_ai::MODEL_WORK_MAXIMUM_INPUT_BYTES
            .max(conduit_ai::MODEL_WORK_MAXIMUM_OUTPUT_BYTES),
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::ModelWork(ModelWorkBack {
        completed: 0,
        pending: false,
    }))
}
