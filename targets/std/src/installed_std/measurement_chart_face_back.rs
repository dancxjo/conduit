//! Ordinary bounded flow sink. Projection is a Host effect; publication waits
//! for the exact completion-consuming Step commit, never stdout or an input label.
use super::back::{BackBudget, BackFactory, InstalledBack};
use crate::{
    body_execution::face_contribution::{FaceCommitGate, FaceCommitObservation},
    measurement_chart_offer::*,
};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallId, PortId, RequestId,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: MEASUREMENT_CHART_FACE_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct MeasurementChartFaceBack {
    gate: FaceCommitGate,
    maximum_updates: u32,
    next_request: u32,
    pending: Option<RequestId>,
    candidate: Option<Candidate>,
    input_bytes: u32,
}
enum Candidate {
    Request(RequestId),
    Completed,
}
impl MeasurementChartFaceBack {
    pub(super) fn observation(&self) -> FaceCommitObservation {
        self.gate.observation()
    }
    pub(super) fn retained_backing_bytes(&self) -> usize {
        self.gate
            .prepared_backing_bytes()
            .expect("checked finite gate backing")
    }
}
impl<const PORTS: usize> StepBack<PORTS> for MeasurementChartFaceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.candidate = None;
        self.gate.begin_step();
        if let Some((request, _)) = io.host_completion() {
            if self.pending != Some(request) || self.gate.consume_completion(io, request).is_err() {
                return failed(FailureCode::InvalidLifecycle);
            }
            self.candidate = Some(Candidate::Completed);
            return StepOutcome::Progress;
        }
        if self.pending.is_some() {
            return StepOutcome::Await;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.next_request >= self.maximum_updates {
                return failed(FailureCode::StorageExhausted);
            }
            let Ok(input) = BoundedValueRef::new(value, self.input_bytes) else {
                return failed(FailureCode::InvalidInput);
            };
            let request = RequestId(self.next_request);
            if io.consume(PortId(0)).is_err()
                || io.request_host_call(request, HostCallId(0), input).is_err()
            {
                return failed(FailureCode::InvalidLifecycle);
            }
            self.candidate = Some(Candidate::Request(request));
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return failed(FailureCode::InvalidLifecycle);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
    fn step_committed(&mut self) {
        match self.candidate.take() {
            Some(Candidate::Request(request)) => {
                self.pending = Some(request);
                self.next_request += 1;
            }
            Some(Candidate::Completed) => {
                self.gate.step_committed();
                self.pending = None;
            }
            None => {}
        }
    }
    fn cancel(&mut self) {
        self.gate.cancel();
        self.pending = None;
        self.candidate = None;
    }
}
fn failed(code: FailureCode) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail: 71 })
}

/// Requested bounded Rust backing for decode, one prepared semantic Face and
/// validation temporaries. Allocator overhead/stacks, input/value storage,
/// actual scope strings and gate/route ownership are accounted separately.
/// This descriptor is not a Root lease or producer authentication.
pub(crate) fn maximum_fragment_backing_bytes() -> usize {
    conduit_std_offers::measurement_chart_face_fragment_backing_bytes()
}
pub fn workspace_bytes() -> u32 {
    conduit_std_offers::measurement_chart_face_workspace_bytes()
}
const _: () = assert!(
    core::mem::size_of::<crate::measurement_face::MeasurementFaceSeries<'static>>()
        == core::mem::size_of::<(&'static str, &'static conduit_data::MeasurementPlotSeries)>()
);
pub(super) fn configuration(placement: &PlannedGear) -> Result<(&str, u32), String> {
    let expected = measurement_chart_face_std_offer(workspace_bytes()).map_err(str::to_owned)?;
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.semantic_contract != expected.semantic_contract
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.host_calls != expected.host_calls
        || placement.limits != expected.limits
        || placement.configuration.len() != 2
    {
        return Err("chart Face differs from installed checked provider".into());
    }
    let get = |name| {
        placement
            .configuration
            .iter()
            .find(|entry| entry.key == name)
            .map(|entry| &entry.value)
    };
    let title = match get("label") {
        Some(ConfigurationValue::Text(title)) if conduit_core::valid_data_label(title) => {
            title.as_str()
        }
        _ => return Err("chart Face requires a bounded authored construction label".into()),
    };
    let maximum = match get("maximum-updates") {
        Some(ConfigurationValue::U64(n)) if (1..=4096).contains(n) => *n as u32,
        _ => return Err("chart Face maximum-updates outside selected finite scope".into()),
    };
    if placement
        .configuration
        .iter()
        .any(|entry| entry.key != "label" && entry.key != "maximum-updates")
    {
        return Err("chart Face has unsupported configuration".into());
    }
    Ok((title, maximum))
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let (_, updates) = configuration(placement)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: conduit_data::measurement_chart_canonical_bytes() as u32,
        host_requests: updates as usize,
        sign_items: u16::try_from(
            updates
                .checked_mul(8)
                .and_then(|n| n.checked_add(16))
                .ok_or("chart Face Sign bound overflow")?,
        )
        .map_err(|_| "chart Face Sign bound exceeds std profile")?,
        maximum_value_bytes: conduit_data::measurement_chart_canonical_bytes() as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let (_, maximum_updates) = configuration(placement)?;
    let gate = FaceCommitGate::prepare(
        placement.placement_id.clone(),
        HostCallId(0),
        maximum_updates,
    )
    .map_err(|e| format!("chart Face gate: {e:?}"))?;
    Ok(InstalledBack::MeasurementChartFace(
        MeasurementChartFaceBack {
            gate,
            maximum_updates,
            next_request: 0,
            pending: None,
            candidate: None,
            input_bytes: conduit_data::measurement_chart_canonical_bytes() as u32,
        },
    ))
}
#[cfg(test)]
#[path = "measurement_chart_face_back/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "measurement_chart_face_back/image_tests.rs"]
mod image_tests;
