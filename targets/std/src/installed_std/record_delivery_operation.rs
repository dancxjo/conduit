//! Installed std projection from bounded delivery observations to exact status.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
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

impl RecordDeliveryStatusOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending
                && self.observations < conduit_net::MAXIMUM_RECORD_DELIVERY_OBSERVATIONS =>
            {
                self.pending = true;
                let Ok(input) = BoundedValueRef::new(
                    value,
                    conduit_net::MAXIMUM_RECORD_DELIVERY_CANONICAL_BYTES as u32,
                ) else {
                    return InstalledOperation::fail(260);
                };
                OperationAction::RequestHostOperation {
                    request: RequestId(u32::from(self.observations)),
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending
                    && request == RequestId(u32::from(self.observations))
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return InstalledOperation::fail(261);
                };
                self.pending = false;
                self.observations += 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(262),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }
}

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
        || placement.host_operations != offer.host_operations
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
