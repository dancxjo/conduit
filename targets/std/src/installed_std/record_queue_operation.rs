//! Allocation-stable std realization of bounded ordered framed-record queueing.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{Failure, FailureCode, OperationAction, OperationInput, PortId, ValueRef};

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

impl RecordQueueOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port: PortId(0) } => OperationAction::Complete,
            _ => fail(FailureCode::InvalidLifecycle, 250),
        }
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        if port != PortId(0) || value.byte_len > MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 {
            return fail(FailureCode::InvalidInput, 251);
        }
        if self.accepted >= self.maximum_items {
            return fail(FailureCode::StorageExhausted, 252);
        }
        let frame = match super::typed_record_operation::typed_leaf(canonical, &self.framed_type) {
            Ok(frame) => frame,
            Err(_) => return fail(FailureCode::InvalidInput, 253),
        };
        if frame.len() > self.maximum_frame_bytes
            || conduit_net::decode_typed_record(frame).is_err()
        {
            return fail(FailureCode::InvalidInput, 254);
        }
        self.accepted += 1;
        OperationAction::Emit {
            port: PortId(0),
            value,
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {}
}

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
        || placement.host_operations != offer.host_operations
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

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
