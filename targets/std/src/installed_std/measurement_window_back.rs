//! Standard admitted wrapper of the shared no_std window state machine/provider.
use super::{
    back::{BackBudget, BackFactory, InstalledBack},
    InstalledScheduler,
};
use conduit_core::PlannedGear;
use conduit_data::{MeasurementWindowStep, PreparedMeasurementWindow, MEASUREMENT_WINDOW_FINALIZE};
use conduit_kernel::{
    scheduler::{HostCallRequest, StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostCallOutcome,
    HostedValueStore, ValueStorage,
};
use conduit_plan_lowering::lowering::LoweredHostCall;
use conduit_std_offers::*;
use std::cell::RefCell;
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: MEASUREMENT_WINDOW_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct MeasurementWindowBack {
    kernel: MeasurementWindowStep,
    provider: RefCell<PreparedMeasurementWindow>,
    failed: bool,
    bounds: [u32; 3],
}
impl MeasurementWindowBack {
    pub(super) fn retained_backing_bytes(&self) -> usize {
        core::mem::size_of::<Self>() + self.provider.borrow().prepared_backing_bytes()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for Box<MeasurementWindowBack> {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.failed {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 37,
            });
        }
        self.kernel.step(io, bytes)
    }
    fn step_committed(&mut self) {
        <MeasurementWindowStep as StepBack<PORTS>>::step_committed(&mut self.kernel);
        if let Some(request) = self.kernel.take_committed_request() {
            if self.provider.get_mut().commit(request).is_err() {
                self.failed = true
            }
        }
    }
    fn cancel(&mut self) {
        <MeasurementWindowStep as StepBack<PORTS>>::cancel(&mut self.kernel);
        self.provider.get_mut().cancel();
        self.failed = true;
    }
}
fn validate(p: &PlannedGear) -> Result<(), String> {
    let expected = measurement_window_std_offer();
    if p.kind_id != expected.kind_id
        || p.kind_contract_revision != expected.kind_contract_revision
        || p.semantic_contract != expected.semantic_contract
        || p.capability_id != expected.capability_id
        || p.implementation_id != expected.implementation.implementation_id
        || p.execution_profile_id != expected.implementation.execution_profile_id
        || p.artifact_id != expected.implementation.artifact_id
        || p.inputs != expected.inputs
        || p.outputs != expected.outputs
        || p.host_calls != expected.host_calls
        || p.limits != expected.limits
        || !p.configuration.is_empty()
    {
        return Err("window placement differs from exact installed profile".into());
    }
    Ok(())
}
fn budget(p: &PlannedGear) -> Result<BackBudget, String> {
    validate(p)?;
    let b = measurement_window_value_bounds();
    Ok(BackBudget {
        value_items: 4,
        value_bytes: b[0] + b[1] + b[2] + MEASUREMENT_WINDOW_FINALIZE.len() as u32,
        host_requests: 2,
        sign_items: 32,
        maximum_value_bytes: b[2].max(b[0]).max(b[1]),
    })
}
fn prepare(p: &PlannedGear, values: &mut HostedValueStore) -> Result<InstalledBack, String> {
    validate(p)?;
    let bounds = measurement_window_value_bounds();
    let finalize = values
        .store(MEASUREMENT_WINDOW_FINALIZE)
        .map_err(|e| format!("window finalizer:{e:?}"))?;
    Ok(InstalledBack::MeasurementWindow(Box::new(
        MeasurementWindowBack {
            kernel: MeasurementWindowStep::new(finalize, HostCallId(0), HostCallId(1), bounds),
            provider: RefCell::new(PreparedMeasurementWindow::new()),
            failed: false,
            bounds,
        },
    )))
}
pub(super) fn supports(op: &LoweredHostCall) -> bool {
    matches!(
        op.contract_id.as_str(),
        MEASUREMENT_WINDOW_PROFILE_CALL | MEASUREMENT_WINDOW_SAMPLE_CALL
    ) && op
        .target_kind
        .as_ref()
        .is_some_and(|k| k.as_str() == conduit_data::MEASUREMENT_COUNT_WINDOW_KIND)
        && op.maximum_in_flight == 1
}
pub(super) fn complete(
    op: &LoweredHostCall,
    request: HostCallRequest,
    scheduler: &mut InstalledScheduler,
) -> Result<bool, String> {
    if !supports(op) {
        return Ok(false);
    }
    if op.node != request.node
        || op.call != request.call
        || request.input.admitted_bytes != op.binding.maximum_input_bytes
    {
        return Err("window request differs from exact operation".into());
    }
    let encoded = {
        let input = scheduler
            .host_value(request.input.value)
            .map_err(|e| format!("window input:{e:?}"))?;
        let Some(InstalledBack::MeasurementWindow(back)) =
            scheduler.drivers().get(usize::from(request.node.0))
        else {
            return Err("window provider absent".into());
        };
        let (call, input_bound, output_bound) =
            if op.contract_id.as_str() == MEASUREMENT_WINDOW_PROFILE_CALL {
                (
                    conduit_data::MEASUREMENT_WINDOW_PROFILE_CALL,
                    back.bounds[0],
                    0,
                )
            } else {
                (
                    conduit_data::MEASUREMENT_WINDOW_SAMPLE_CALL,
                    back.bounds[1],
                    back.bounds[2],
                )
            };
        if op.binding.maximum_input_bytes != input_bound
            || op.binding.maximum_output_bytes != output_bound
        {
            return Err("window operation bounds differ".into());
        }
        back.provider
            .borrow_mut()
            .execute(request.request, call, input)
            .map(|bytes| bytes.map(<[u8]>::to_vec))
    };
    let outcome = match encoded {
        Ok(bytes) => HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: match bytes {
                Some(bytes) => Some(
                    BoundedValueRef::new(
                        scheduler
                            .store_host_value(&bytes)
                            .map_err(|e| format!("window output:{e:?}"))?,
                        op.binding.maximum_output_bytes,
                    )
                    .map_err(|e| format!("window output bound:{e:?}"))?,
                ),
                None => None,
            },
            failure: None,
        },
        Err(failure) => HostCallOutcome {
            disposition: HostCallDisposition::Failed,
            output: None,
            failure: Some(failure),
        },
    };
    scheduler
        .complete_host_call(request.node, request.request, outcome)
        .map_err(|e| format!("window completion:{e:?}"))?;
    Ok(true)
}
#[cfg(test)]
#[path = "measurement_window_back/tests.rs"]
mod tests;
