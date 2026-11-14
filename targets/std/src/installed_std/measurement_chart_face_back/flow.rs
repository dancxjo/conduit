//! Explicit terminal-capable child of the ordinary chart Face provider.
//! A terminal-only output preserves the real input terminal without inventing
//! payload values or laundering semantic trouble into mechanism failure.
use super::*;
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction,
    },
    CanonicalValue,
};

pub(in crate::installed_std) struct MeasurementChartFlowFaceBack {
    inner: MeasurementChartFaceBack,
    abnormal_candidate: bool,
}
impl MeasurementChartFlowFaceBack {
    pub(in crate::installed_std) fn observation(&self) -> FaceCommitObservation {
        self.inner.observation()
    }
    pub(in crate::installed_std) fn retained_backing_bytes(&self) -> usize {
        self.inner.retained_backing_bytes()
    }
    fn abnormal<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        terminal: CanonicalValue,
    ) -> StepOutcome {
        if conduit_core::TerminalInfo::decode(terminal.as_slice()).is_err()
            || io.consume_abnormal(PortId(0)).is_err()
        {
            return failed(FailureCode::InvalidInput);
        }
        self.abnormal_candidate = true;
        StepOutcome::Abnormal {
            port: PortId(0),
            terminal,
        }
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
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.abnormal_candidate = false;
        if io.host_completion().is_some() {
            // Kernel blocks this Back until genuine completion exists. Consume
            // that completion AND exposed terminal in the SAME transaction.
            let outcome = self.inner.step(io, inputs);
            if outcome != StepOutcome::Progress {
                return outcome;
            }
            if io.input(PortId(0)).is_none() {
                if let Some(terminal) = io.input_abnormal(PortId(0)) {
                    return self.abnormal(io, terminal);
                }
                if io.input_closed(PortId(0)) {
                    if io.consume_closed(PortId(0)).is_err() {
                        return failed(FailureCode::InvalidLifecycle);
                    }
                    self.abnormal_candidate = true;
                    return StepOutcome::Complete;
                }
            }
            return outcome;
        }
        self.inner.candidate = None;
        self.inner.gate.begin_step();
        if self.inner.pending.is_none() && io.input(PortId(0)).is_none() {
            if let Some(terminal) = io.input_abnormal(PortId(0)) {
                return self.abnormal(io, terminal);
            }
        }
        let outcome = self.inner.step(io, inputs);
        if outcome == StepOutcome::Complete {
            self.abnormal_candidate = true;
        }
        outcome
    }
    fn step_committed(&mut self) {
        <MeasurementChartFaceBack as StepBack<PORTS>>::step_committed(&mut self.inner);
        if self.abnormal_candidate {
            // No call remains owed: normal or abnormal terminal committed.
            // This same gate's shared witness retires every escaped old frame.
            self.inner.gate.cancel();
            self.abnormal_candidate = false;
        }
    }
    fn cancel(&mut self) {
        self.abnormal_candidate = false;
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
        host_requests: updates as usize,
        sign_items: u16::try_from(
            updates
                .checked_mul(8)
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
            abnormal_candidate: false,
        },
    ))
}
