//! Installed std projection from bounded delivery observations to exact status.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::RECORD_DELIVERY_STATUS_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct RecordDeliveryStatusOperation {
    pending: bool,
    observations: u16,
}

impl<const PORTS: usize> StepOperation<PORTS> for RecordDeliveryStatusOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(u32::from(self.observations)) {
                return step_fail(262);
            }
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            if outcome.disposition != HostCallDisposition::Completed {
                return step_fail(262);
            }
            let Some(output) = outcome.output else {
                return step_fail(261);
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_host_completion()
                .expect("observed record delivery completion");
            io.send(PortId(0), output.value)
                .expect("ready record delivery status output");
            self.pending = false;
            self.observations += 1;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending
                || self.observations >= conduit_net::MAXIMUM_RECORD_DELIVERY_OBSERVATIONS
            {
                return step_fail(262);
            }
            let Ok(input) = BoundedValueRef::new(
                value,
                conduit_net::MAXIMUM_RECORD_DELIVERY_CANONICAL_BYTES as u32,
            ) else {
                return step_fail(260);
            };
            let request = RequestId(u32::from(self.observations));
            io.consume(PortId(0))
                .expect("present record delivery observation");
            io.request_host_call(request, HostCallId(0), input)
                .expect("record delivery Host Call");
            self.pending = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && !self.pending {
            io.consume_closed(PortId(0))
                .expect("observed record delivery closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

const fn step_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

impl RecordDeliveryStatusOperation {}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Result<Vec<Option<conduit_net::BoundedRecordDeliveryStatusCodec>>, String> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_std_offers::RECORD_DELIVERY_STATUS_STD_IMPLEMENTATION
            {
                conduit_net::BoundedRecordDeliveryStatusCodec::prepare()
                    .map(Some)
                    .map_err(|error| format!("prepare record delivery status: {error:?}"))
            } else {
                Ok(None)
            }
        })
        .collect()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::record_delivery_status_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned delivery status differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: conduit_net::MAXIMUM_RECORD_DELIVERY_OBSERVATIONS,
        value_bytes: (conduit_net::MAXIMUM_RECORD_DELIVERY_CANONICAL_BYTES as u32)
            * u32::from(conduit_net::MAXIMUM_RECORD_DELIVERY_OBSERVATIONS),
        host_requests: conduit_net::MAXIMUM_RECORD_DELIVERY_OBSERVATIONS.into(),
        sign_items: conduit_net::MAXIMUM_RECORD_DELIVERY_OBSERVATIONS.saturating_mul(6) + 8,
        maximum_value_bytes: conduit_net::MAXIMUM_RECORD_DELIVERY_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::RecordDeliveryStatus(
        RecordDeliveryStatusOperation {
            pending: false,
            observations: 0,
        },
    ))
}

pub(super) fn refusal_detail(refusal: conduit_net::RecordDeliveryRefusal) -> u16 {
    use conduit_net::RecordDeliveryRefusal::*;
    match refusal {
        EmptyCorrelation => 1,
        CorrelationTooLong => 2,
        EmptyFrame => 3,
        FrameTooLarge => 4,
        InvalidTransition => 5,
        InvalidPartialProgress => 6,
        EmptyReceipt => 7,
        ReceiptTooLong => 8,
        OutputTooSmall => 9,
        MalformedWire => 10,
        ObservationIdentityMismatch => 11,
    }
}
