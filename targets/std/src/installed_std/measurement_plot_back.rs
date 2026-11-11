//! Installed ordinary Host Call realization. Projection allocates bounded
//! temporaries outside the sealed Step; it is never an ImmediateNativeSource.
use super::{
    InstalledScheduler,
    back::{BackBudget, BackFactory, InstalledBack},
};
use crate::hosted_measurement::*;
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallOutcome,
    scheduler::{HostCallBack, HostCallRequest, StepBack, StepInputBytes, StepIo, StepOutcome},
};
use conduit_plan_lowering::lowering::LoweredHostCall;
use std::cell::RefCell;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: MEASUREMENT_PLOT_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct MeasurementPlotBack {
    kernel: HostCallBack,
    provider: RefCell<PreparedMeasurementPlot>,
    input_bound: u32,
    output_bound: u32,
}
impl MeasurementPlotBack {
    pub(super) fn retained_backing_bytes(&self) -> usize {
        core::mem::size_of::<Self>() + self.provider.borrow().retained_backing_bytes()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for Box<MeasurementPlotBack> {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.kernel.step(io, bytes)
    }
    fn cancel(&mut self) {
        <HostCallBack as StepBack<PORTS>>::cancel(&mut self.kernel)
    }
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    // Exact provider/profile validation happens in both budget and preparation.
    // The temporary provider belongs to preparation, never the sealed Step.
    PreparedMeasurementPlot::prepare(placement)?;
    let (input, output) = measurement_plot_value_bounds();
    Ok(BackBudget {
        value_items: 2,
        value_bytes: input
            .checked_add(output)
            .ok_or("measurement values overflow")?,
        host_requests: 1,
        sign_items: 32,
        maximum_value_bytes: input.max(output),
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let (kernel, provider) = PreparedMeasurementPlot::prepare(placement)?;
    let (input_bound, output_bound) = measurement_plot_value_bounds();
    Ok(InstalledBack::MeasurementPlot(Box::new(
        MeasurementPlotBack {
            kernel,
            provider: RefCell::new(provider),
            input_bound,
            output_bound,
        },
    )))
}
pub(super) fn supports(operation: &LoweredHostCall) -> bool {
    operation.contract_id.as_str() == MEASUREMENT_PLOT_HOST_CALL
        && operation
            .target_kind
            .as_ref()
            .is_some_and(|kind| kind.as_str() == conduit_data::MEASUREMENT_PLOT_KIND)
        && operation.maximum_in_flight == 1
}
/// Correlation, input/output bounds and actual installed driver are checked
/// before provider invocation. The scheduler owns completion/cancellation.
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
        return Err("measurement request differs from selected operation".into());
    }
    let encoded = {
        let input = scheduler
            .host_value(request.input.value)
            .map_err(|e| format!("measurement input: {e:?}"))?;
        let Some(InstalledBack::MeasurementPlot(back)) =
            scheduler.drivers().get(usize::from(request.node.0))
        else {
            return Err("measurement request has no exact installed provider".into());
        };
        if operation.binding.maximum_input_bytes != back.input_bound
            || operation.binding.maximum_output_bytes != back.output_bound
        {
            return Err("measurement bounds differ from installed provider".into());
        }
        // This <=output-bound copy releases the driver/input borrow before
        // insertion into kernel-owned storage. Its peak is in the workspace.
        back.provider
            .borrow_mut()
            .execute(input)
            .map(<[u8]>::to_vec)
    };
    let outcome = match encoded {
        Ok(bytes) => {
            let value = scheduler
                .store_host_value(&bytes)
                .map_err(|e| format!("measurement output: {e:?}"))?;
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(value, operation.binding.maximum_output_bytes)
                        .map_err(|e| format!("measurement output bound: {e:?}"))?,
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
        .map_err(|e| format!("measurement completion: {e:?}"))?;
    Ok(true)
}
#[cfg(test)]
#[path = "measurement_plot_back/tests.rs"]
mod tests;
