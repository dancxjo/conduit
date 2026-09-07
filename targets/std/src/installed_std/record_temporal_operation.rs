//! Pure-kernel std realizations of explicit framed-record temporal boundaries.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{OperationAction, OperationInput, PortId, ValueRef};

pub(super) static SINGLETON: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::RECORD_SINGLETON_STREAM_STD_IMPLEMENTATION,
    budget,
    prepare: prepare_singleton,
};
pub(super) static EXACTLY_ONE: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::RECORD_EXACTLY_ONE_STD_IMPLEMENTATION,
    budget,
    prepare: prepare_exactly_one,
};

pub(super) struct RecordSingletonStreamOperation {
    emitted: bool,
}

impl RecordSingletonStreamOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.emitted && value.byte_len <= MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 => {
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            _ => InstalledOperation::fail(240),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Complete
    }

    pub(super) fn cancel(&mut self) {
        self.emitted = true;
    }
}

pub(super) struct RecordExactlyOneOperation {
    held: Option<ValueRef>,
    released: Option<ValueRef>,
    emitted: bool,
    retain_resumed: bool,
}

impl RecordExactlyOneOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.held.is_none()
                && value.byte_len <= MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 =>
            {
                self.held = Some(value);
                self.retain_resumed = true;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if !self.emitted => {
                let Some(value) = self.held.take() else {
                    return InstalledOperation::fail(241);
                };
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            _ => InstalledOperation::fail(241),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Complete
    }

    pub(super) fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.take()
    }

    pub(super) fn cancel(&mut self) {
        self.released = self.held.take();
        self.retain_resumed = false;
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare_singleton(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::RecordSingletonStream(
        RecordSingletonStreamOperation { emitted: false },
    ))
}

fn prepare_exactly_one(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::RecordExactlyOne(
        RecordExactlyOneOperation {
            held: None,
            released: None,
            emitted: false,
            retain_resumed: false,
        },
    ))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = match placement.implementation_id.as_str() {
        conduit_std_offers::RECORD_SINGLETON_STREAM_STD_IMPLEMENTATION => {
            conduit_std_offers::record_singleton_stream_std_offer()
        }
        conduit_std_offers::RECORD_EXACTLY_ONE_STD_IMPLEMENTATION => {
            conduit_std_offers::record_exactly_one_std_offer()
        }
        _ => return Err("unknown std record temporal implementation".into()),
    };
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
        return Err("planned record temporal Gear differs from installed realization".into());
    }
    Ok(())
}
