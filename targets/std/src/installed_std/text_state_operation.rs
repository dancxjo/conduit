//! Installed bounded retained text editing and repeated line submission.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static EDIT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TEXT_EDIT_STD_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static SUBMIT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TEXT_SUBMIT_LINES_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TextStateOperation {
    pending: Option<RequestId>,
    next_request: u32,
}

impl<const PORTS: usize> StepBack<PORTS> for TextStateOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 4);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed text state Host Call completion");
                    io.send(PortId(0), output.value)
                        .expect("ready text state output");
                    self.pending = None;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed text state Host Call completion");
                    self.pending = None;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Cancelled, None, None) => {
                    io.consume_host_completion()
                        .expect("observed cancelled text state Host Call");
                    self.pending = None;
                    step_fail(FailureCode::Cancelled, 0)
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    io.consume_host_completion()
                        .expect("observed failed text state Host Call");
                    self.pending = None;
                    StepOutcome::Fail(failure)
                }
                _ => step_fail(FailureCode::InvalidLifecycle, 3),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return step_fail(FailureCode::InvalidLifecycle, 4);
            }
            let Ok(input) = BoundedValueRef::new(value, 4) else {
                return step_fail(FailureCode::InvalidInput, 2);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_fail(FailureCode::StorageExhausted, 1);
            };
            io.consume(PortId(0)).expect("present text state input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("single text state Host Call");
            self.next_request = next;
            self.pending = Some(request);
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed text state input closure");
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl TextStateOperation {}

pub(super) struct TextStateHost(conduit_semantic_catalog::BoundedTextState);

impl TextStateHost {
    pub(super) fn from_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        let (mode, offer) = match placement.implementation_id.as_str() {
            conduit_std_offers::TEXT_EDIT_STD_IMPLEMENTATION => (
                conduit_semantic_catalog::TextStateMode::Edit,
                conduit_std_offers::text_edit_std_offer(),
            ),
            conduit_std_offers::TEXT_SUBMIT_LINES_STD_IMPLEMENTATION => (
                conduit_semantic_catalog::TextStateMode::Submit,
                conduit_std_offers::text_submit_lines_std_offer(),
            ),
            _ => return Ok(None),
        };
        validate(placement, &offer)?;
        let maximum = maximum_bytes(placement)?;
        conduit_semantic_catalog::BoundedTextState::new(mode, maximum)
            .map(Self)
            .map(Some)
            .map_err(|_| "text state capacity is outside the portable bound".into())
    }

    pub(super) fn execute(
        &mut self,
        fragment: &[u8],
    ) -> Result<Option<&[u8]>, conduit_semantic_catalog::TextStateRefusal> {
        self.0.apply(fragment)
    }
}

fn expected_offer(placement: &PlannedGear) -> Result<conduit_core::CapabilityOffer, String> {
    match placement.implementation_id.as_str() {
        conduit_std_offers::TEXT_EDIT_STD_IMPLEMENTATION => {
            Ok(conduit_std_offers::text_edit_std_offer())
        }
        conduit_std_offers::TEXT_SUBMIT_LINES_STD_IMPLEMENTATION => {
            Ok(conduit_std_offers::text_submit_lines_std_offer())
        }
        _ => Err("unsupported text state implementation".into()),
    }
}

fn validate(placement: &PlannedGear, offer: &conduit_core::CapabilityOffer) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned text state differs from installed realization".into());
    }
    Ok(())
}

fn maximum_bytes(placement: &PlannedGear) -> Result<usize, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            ("maximum-bytes", ConfigurationValue::U64(value)) => usize::try_from(*value).ok(),
            _ => None,
        })
        .filter(|maximum| {
            (1..=conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES as usize).contains(maximum)
        })
        .ok_or_else(|| "text state maximum-bytes is invalid".into())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let offer = expected_offer(placement)?;
    validate(placement, &offer)?;
    let maximum = maximum_bytes(placement)? as u32;
    Ok(OperationBudget {
        value_items: 8,
        value_bytes: maximum.saturating_mul(8),
        host_requests: 1,
        sign_items: 64,
        maximum_value_bytes: maximum,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    budget(placement)?;
    Ok(InstalledOperation::TextState(TextStateOperation {
        pending: None,
        next_request: 0,
    }))
}
