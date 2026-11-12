//! Pure stateless chart construction; latest state remains in combine-latest.
use super::{
    back::{BackBudget, BackFactory, InstalledBack},
    InstalledScheduler,
};
use crate::hosted_measurement_chart_constructor::*;
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedFiniteTerminalEmission, AssignedNormalCloseTransduction,
        AssignedTerminalTransduction, HostCallBack, HostCallRequest, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    BoundedValueRef, HostCallDisposition, HostCallOutcome, PortId,
};
use conduit_plan_lowering::lowering::LoweredHostCall;
use std::cell::RefCell;
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: MEASUREMENT_CHART_CONSTRUCTOR_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct MeasurementChartConstructorBack {
    kernel: HostCallBack,
    provider: RefCell<PreparedMeasurementChartConstructor>,
    input_bound: u32,
    output_bound: u32,
    workspace: u32,
    maximum_updates: u32,
    updates: u32,
    requested_candidate: bool,
}
impl MeasurementChartConstructorBack {
    pub(super) fn workspace_bytes(&self) -> u32 {
        self.workspace
    }
    pub(super) fn retained_backing_bytes(&self) -> usize {
        core::mem::size_of::<Self>() + self.provider.borrow().retained_backing_bytes()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for Box<MeasurementChartConstructorBack> {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        Some(AssignedTerminalTransduction {
            input: PortId(0),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::FlushThenPropagate(
                AssignedFiniteTerminalEmission {
                    maximum_items: 1,
                    maximum_bytes: self.output_bound,
                },
            ),
            abnormal: AssignedAbnormalTransduction::FinalizeThenPropagate(
                AssignedFiniteTerminalEmission {
                    maximum_items: 1,
                    maximum_bytes: self.output_bound,
                },
            ),
            cancellation: AssignedCancellationTransduction::NotCancellable,
        })
    }
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.requested_candidate = false;
        let completion = io.host_completion().is_some();
        let requesting = !completion && io.input(PortId(0)).is_some();
        if requesting && self.updates >= self.maximum_updates {
            return StepOutcome::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::StorageExhausted,
                detail: 72,
            });
        }
        if !completion {
            if let Some(terminal) = io.input_abnormal(PortId(0)) {
                io.consume_abnormal(PortId(0))
                    .expect("observed typed terminal");
                return StepOutcome::Abnormal {
                    port: PortId(0),
                    terminal,
                };
            }
        }
        let outcome = self.kernel.step(io, bytes);
        self.requested_candidate = requesting && outcome == StepOutcome::Progress;
        if completion && outcome == StepOutcome::Progress {
            if let Some(terminal) = io.input_abnormal(PortId(0)) {
                io.consume_abnormal(PortId(0))
                    .expect("observed typed terminal");
                return StepOutcome::Abnormal {
                    port: PortId(0),
                    terminal,
                };
            }
            if io.input_closed(PortId(0)) {
                io.consume_closed(PortId(0)).expect("observed normal close");
                return StepOutcome::Complete;
            }
        }
        outcome
    }

    fn step_committed(&mut self) {
        if self.requested_candidate {
            self.updates += 1;
            self.requested_candidate = false;
        }
    }
    fn cancel(&mut self) {
        self.requested_candidate = false;
        <HostCallBack as StepBack<PORTS>>::cancel(&mut self.kernel)
    }
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let provider = PreparedMeasurementChartConstructor::prepare(placement)?;
    let input = provider.input_bound();
    let output = conduit_data::measurement_chart_canonical_bytes() as u32;
    let updates =
        conduit_data::measurement_chart_constructor_maximum_updates(&placement.configuration)?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: input + output,
        host_requests: updates as usize,
        sign_items: u16::try_from(
            updates
                .checked_mul(8)
                .and_then(|n| n.checked_add(24))
                .ok_or("chart Sign bound overflow")?,
        )
        .map_err(|_| "chart Sign bound exceeds std profile")?,
        maximum_value_bytes: input.max(output),
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let provider = PreparedMeasurementChartConstructor::prepare(placement)?;
    let input_bound = provider.input_bound();
    let output_bound = provider.output_bound();
    let workspace = provider.workspace_bytes();
    Ok(InstalledBack::MeasurementChartConstructor(Box::new(
        MeasurementChartConstructorBack {
            kernel: HostCallBack::new(input_bound),
            provider: RefCell::new(provider),
            output_bound,
            workspace,
            maximum_updates: conduit_data::measurement_chart_constructor_maximum_updates(
                &placement.configuration,
            )?,
            updates: 0,
            requested_candidate: false,
            input_bound,
        },
    )))
}
pub(super) fn supports(operation: &LoweredHostCall) -> bool {
    operation.contract_id.as_str() == MEASUREMENT_CHART_CONSTRUCTOR_CALL
        && operation.target_kind.as_ref().is_some_and(|kind| {
            conduit_data::measurement_chart_constructor_arity(kind.as_str()).is_some()
        })
        && operation.maximum_in_flight == 1
}

pub(super) fn complete(
    operation: &LoweredHostCall,
    request: HostCallRequest,
    scheduler: &mut InstalledScheduler,
) -> Result<bool, String> {
    if !supports(operation) {
        return Ok(false);
    }
    if operation.node != request.node
        || operation.call != request.call
        || request.input.admitted_bytes != operation.binding.maximum_input_bytes
    {
        return Err("chart constructor request differs from selected operation".into());
    }
    let result = {
        let input = scheduler
            .host_value(request.input.value)
            .map_err(|e| format!("chart input: {e:?}"))?;
        let Some(InstalledBack::MeasurementChartConstructor(back)) =
            scheduler.drivers().get(usize::from(request.node.0))
        else {
            return Err("chart constructor has no prepared driver".into());
        };
        if operation.binding.maximum_input_bytes != back.input_bound
            || operation.binding.maximum_output_bytes != back.output_bound
        {
            return Err("chart constructor bounds differ".into());
        }
        back.provider
            .borrow_mut()
            .execute(input)
            .map(<[u8]>::to_vec)
    };
    let outcome = match result {
        Ok(bytes) => {
            let value = scheduler
                .store_host_value(&bytes)
                .map_err(|e| format!("chart output: {e:?}"))?;
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(value, operation.binding.maximum_output_bytes)
                        .map_err(|e| format!("chart bound: {e:?}"))?,
                ),
                failure: None,
            }
        }
        Err(failure) => HostCallOutcome {
            disposition: HostCallDisposition::Failed,
            output: None,
            failure: Some(failure),
        },
    };
    scheduler
        .complete_host_call(request.node, request.request, outcome)
        .map_err(|e| format!("chart completion: {e:?}"))?;
    Ok(true)
}
