//! Pure-kernel std realizations of explicit framed-record temporal boundaries.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};

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

impl<const PORTS: usize> StepBack<PORTS> for RecordSingletonStreamOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        let Some(value) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        if value.byte_len > MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 {
            return StepOutcome::Fail(step_failure(240));
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.consume(PortId(0))
            .expect("present singleton record input");
        io.send(PortId(0), value)
            .expect("ready singleton record output");
        self.emitted = true;
        StepOutcome::Progress
    }
}

impl RecordSingletonStreamOperation {}

pub(super) struct RecordExactlyOneOperation {
    held: Option<ValueRef>,
    released: Option<ValueRef>,
    emitted: bool,
    retain_resumed: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for RecordExactlyOneOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if self.held.is_none() {
            if let Some(value) = io.input(PortId(0)) {
                if value.byte_len > MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 {
                    return StepOutcome::Fail(step_failure(241));
                }
                self.held = Some(
                    io.take_input(PortId(0))
                        .expect("present exactly-one record input"),
                );
                return StepOutcome::Progress;
            }
        } else if io.input(PortId(0)).is_some() {
            return StepOutcome::Fail(step_failure(241));
        }
        if io.input_closed(PortId(0)) {
            let Some(value) = self.held else {
                return StepOutcome::Fail(step_failure(241));
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_closed(PortId(0))
                .expect("observed exactly-one record closure");
            io.send(PortId(0), value)
                .expect("ready exactly-one record output");
            self.held = None;
            self.emitted = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.held = None;
        self.released = None;
        self.retain_resumed = false;
    }
}

const fn step_failure(detail: u16) -> conduit_kernel::Failure {
    conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    }
}

impl RecordExactlyOneOperation {}

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
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned record temporal Gear differs from installed realization".into());
    }
    Ok(())
}
