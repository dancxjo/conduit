//! Explicit terminal-capable child of the ordinary chart Face provider.
//! A terminal-only output preserves the real input terminal without inventing
//! payload values or laundering semantic trouble into mechanism failure.
use super::*;
use conduit_kernel::scheduler::{
    AssignedAbnormalTransduction, AssignedCancellationTransduction,
    AssignedNormalCloseTransduction, AssignedTerminalTransduction,
};

// The original chart stays on its admitted Cord until observer custody is
// accepted. Its queued payload shields the following terminal naturally.
#[derive(Clone, Copy)]
struct Phase {
    request: RequestId,
    value: conduit_kernel::ValueRef,
    custody: bool,
}
enum FlowCandidate {
    Phase(Phase),
    Finished,
    Retired,
}
pub(in crate::installed_std) struct MeasurementChartFlowFaceBack {
    inner: MeasurementChartFaceBack,
    phase: Option<Phase>,
    staged: Option<FlowCandidate>,
    cancelled: bool,
}
impl MeasurementChartFlowFaceBack {
    pub(in crate::installed_std) fn observation(&self) -> FaceCommitObservation {
        self.inner.observation()
    }
    pub(in crate::installed_std) fn retained_backing_bytes(&self) -> usize {
        self.inner.retained_backing_bytes()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for MeasurementChartFlowFaceBack {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        Some(AssignedTerminalTransduction {
            input: PortId(0),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::PropagateAfterDrain,
            abnormal: AssignedAbnormalTransduction::PropagateAfterDrain,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        })
    }
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = None;
        if self.cancelled {
            return failed(FailureCode::InvalidLifecycle);
        }
        self.inner.gate.begin_step();
        if let Some(phase) = self.phase {
            if io.input(PortId(0)) != Some(phase.value) {
                return failed(FailureCode::InvalidLifecycle);
            }
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != phase.request {
                return failed(FailureCode::InvalidLifecycle);
            }
            if phase.custody {
                if outcome.disposition != conduit_kernel::HostCallDisposition::Completed
                    || outcome.output.is_some()
                    || outcome.failure.is_some()
                    || io.consume_host_completion().is_err()
                    || io.consume(PortId(0)).is_err()
                {
                    return failed(FailureCode::InvalidLifecycle);
                }
                self.staged = Some(FlowCandidate::Finished);
            } else {
                if self.inner.gate.consume_completion(io, request).is_err() {
                    return failed(FailureCode::InvalidLifecycle);
                }
                let Some(next) = request.0.checked_add(1).map(RequestId) else {
                    return failed(FailureCode::StorageExhausted);
                };
                let Ok(input) = BoundedValueRef::new(phase.value, self.inner.input_bytes) else {
                    return failed(FailureCode::InvalidInput);
                };
                if io.borrow_input_for_call(PortId(0)).is_err()
                    || io.request_host_call(next, HostCallId(1), input).is_err()
                {
                    return failed(FailureCode::InvalidLifecycle);
                }
                self.staged = Some(FlowCandidate::Phase(Phase {
                    request: next,
                    custody: true,
                    ..phase
                }));
            }
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.inner.next_request >= self.inner.maximum_updates {
                return failed(FailureCode::StorageExhausted);
            }
            let Some(request) = self.inner.next_request.checked_mul(2).map(RequestId) else {
                return failed(FailureCode::StorageExhausted);
            };
            let Ok(input) = BoundedValueRef::new(value, self.inner.input_bytes) else {
                return failed(FailureCode::InvalidInput);
            };
            if io.borrow_input_for_call(PortId(0)).is_err()
                || io.request_host_call(request, HostCallId(0), input).is_err()
            {
                return failed(FailureCode::InvalidLifecycle);
            }
            self.staged = Some(FlowCandidate::Phase(Phase {
                request,
                value,
                custody: false,
            }));
            return StepOutcome::Progress;
        }
        if let Some(terminal) = io.input_abnormal(PortId(0)) {
            if conduit_core::TerminalInfo::decode(terminal.as_slice()).is_err()
                || io.consume_abnormal(PortId(0)).is_err()
            {
                return failed(FailureCode::InvalidInput);
            }
            self.staged = Some(FlowCandidate::Retired);
            return StepOutcome::Abnormal {
                port: PortId(0),
                terminal,
            };
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return failed(FailureCode::InvalidLifecycle);
            }
            self.staged = Some(FlowCandidate::Retired);
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
    fn step_committed(&mut self) {
        self.inner.gate.step_committed();
        match self.staged.take() {
            Some(FlowCandidate::Phase(phase)) => self.phase = Some(phase),
            Some(FlowCandidate::Finished) => {
                self.phase = None;
                self.inner.next_request += 1;
            }
            Some(FlowCandidate::Retired) => self.inner.gate.cancel(),
            None => {}
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.phase = None;
        self.staged = None;
        <MeasurementChartFaceBack as StepBack<PORTS>>::cancel(&mut self.inner);
    }
}

pub(in crate::installed_std) static FACTORY: BackFactory = BackFactory {
    implementation_id:
        crate::measurement_chart_flow_offer::MEASUREMENT_CHART_FLOW_FACE_IMPLEMENTATION,
    budget: budget_flow,
    prepare: prepare_flow,
};
pub(in crate::installed_std) fn configuration(
    placement: &PlannedGear,
) -> Result<(&str, u32), String> {
    let expected = crate::measurement_chart_flow_offer::measurement_chart_flow_face_std_offer(
        super::workspace_bytes(),
    )
    .map_err(str::to_owned)?;
    super::configuration_for(placement, &expected)
}
fn budget_flow(placement: &PlannedGear) -> Result<BackBudget, String> {
    let (_, updates) = configuration(placement)?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: (conduit_data::measurement_chart_canonical_bytes()
            + conduit_core::TERMINAL_INFO_ENCODED_LEN) as u32,
        host_requests: updates as usize * 2,
        sign_items: u16::try_from(
            updates
                .checked_mul(16)
                .and_then(|n| n.checked_add(24))
                .ok_or("chart flow Face Sign bound overflow")?,
        )
        .map_err(|_| "chart flow Face Sign scope exceeds std profile")?,
        maximum_value_bytes: conduit_data::measurement_chart_canonical_bytes() as u32,
    })
}
fn prepare_flow(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let (_, maximum_updates) = configuration(placement)?;
    let gate = FaceCommitGate::prepare(
        placement.placement_id.clone(),
        HostCallId(0),
        maximum_updates,
    )
    .map_err(|e| format!("chart flow Face gate: {e:?}"))?;
    Ok(InstalledBack::MeasurementChartFlowFace(
        MeasurementChartFlowFaceBack {
            inner: MeasurementChartFaceBack {
                gate,
                maximum_updates,
                next_request: 0,
                pending: None,
                candidate: None,
                input_bytes: conduit_data::measurement_chart_canonical_bytes() as u32,
            },
            phase: None,
            staged: None,
            cancelled: false,
        },
    ))
}

#[cfg(test)]
#[path = "flow/tests.rs"]
mod tests;
