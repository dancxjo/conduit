//! Allocation-stable std realization of bounded ordered framed-record queueing.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::ORDERED_RECORD_QUEUE_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct RecordQueueOperation {
    maximum_items: u64,
    maximum_frame_bytes: usize,
    accepted: u64,
    framed_type: Vec<u8>,
}

impl<const PORTS: usize> StepBack<PORTS> for RecordQueueOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if value.byte_len > MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 {
                return step_fail(FailureCode::InvalidInput, 251);
            }
            if self.accepted >= self.maximum_items {
                return step_fail(FailureCode::StorageExhausted, 252);
            }
            let Some(canonical) = input_bytes.input(PortId(0)) else {
                return step_fail(FailureCode::InvalidInput, 251);
            };
            let frame =
                match super::typed_record_operation::typed_leaf(canonical, &self.framed_type) {
                    Ok(frame) => frame,
                    Err(_) => return step_fail(FailureCode::InvalidInput, 253),
                };
            if frame.len() > self.maximum_frame_bytes
                || conduit_net::decode_typed_record(frame).is_err()
            {
                return step_fail(FailureCode::InvalidInput, 254);
            }
            io.consume(PortId(0)).expect("present queued record");
            io.send(PortId(0), value)
                .expect("ready ordered-record output");
            self.accepted += 1;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed ordered-record closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl RecordQueueOperation {}

fn limits(placement: &PlannedGear) -> Result<(u64, usize), String> {
    let [items, bytes] = placement.configuration.as_slice() else {
        return Err("ordered record queue requires two exact bounds".into());
    };
    let ("maximum-items", ConfigurationValue::U64(items)) = (&*items.key, &items.value) else {
        return Err("ordered record queue item bound is invalid".into());
    };
    let ("maximum-frame-bytes", ConfigurationValue::U64(bytes)) = (&*bytes.key, &bytes.value)
    else {
        return Err("ordered record queue frame bound is invalid".into());
    };
    let bytes = usize::try_from(*bytes).map_err(|_| "ordered record queue byte overflow")?;
    if *items == 0
        || *items > conduit_net::MAXIMUM_ORDERED_RECORD_QUEUE_ITEMS as u64
        || !(conduit_net::TYPED_RECORD_FRAME_HEADER_BYTES
            ..=conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES)
            .contains(&bytes)
    {
        return Err("ordered record queue bounds are outside reviewed limits".into());
    }
    Ok((*items, bytes))
}

fn validate(placement: &PlannedGear) -> Result<(u64, usize), String> {
    let offer = conduit_std_offers::ordered_record_queue_std_offer();
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
        return Err("planned record queue differs from installed realization".into());
    }
    limits(placement)
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let (items, _) = validate(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: u16::try_from(items.saturating_mul(6) + 8)
            .map_err(|_| "record queue sign overflow")?,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    let (maximum_items, maximum_frame_bytes) = validate(placement)?;
    Ok(InstalledOperation::RecordQueue(RecordQueueOperation {
        maximum_items,
        maximum_frame_bytes,
        accepted: 0,
        framed_type: conduit_net::framed_typed_record_type()
            .canonical_bytes()
            .map_err(|error| format!("encode framed-record type: {error:?}"))?,
    }))
}
