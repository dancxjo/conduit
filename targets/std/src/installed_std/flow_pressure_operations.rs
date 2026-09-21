use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDirection, PortTemporal};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    OperationAction, OperationInput, PortId,
};

pub(super) static FLOW_BACKPRESSURE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::FLOW_BACKPRESSURE_STD_IMPLEMENTATION,
    budget: backpressure_budget,
    prepare: prepare_backpressure,
};

pub(super) static FLOW_COALESCE_LATEST_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::FLOW_COALESCE_LATEST_STD_IMPLEMENTATION,
    budget: coalesce_budget,
    prepare: prepare_coalesce,
};

pub(super) struct FlowPressureOperation;

impl<const PORTS: usize> StepOperation<PORTS> for FlowPressureOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(0)).expect("present flow input");
            io.send(PortId(0), value).expect("ready flow output");
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed flow input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

impl FlowPressureOperation {
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } => OperationAction::Emit {
                port: PortId(0),
                value,
            },
            OperationInput::Closed { port: PortId(0) } => OperationAction::Complete,
            _ => InstalledOperation::fail(270),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }
}

fn backpressure_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_backpressure(placement)?;
    Ok(budget(placement))
}

fn coalesce_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_coalesce(placement)?;
    Ok(budget(placement))
}

fn budget(placement: &PlannedGear) -> OperationBudget {
    OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: placement.limits.max_queue_bytes,
    }
}

fn prepare_backpressure(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_backpressure(placement)?;
    Ok(InstalledOperation::FlowBackpressure(FlowPressureOperation))
}

fn prepare_coalesce(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_coalesce(placement)?;
    Ok(InstalledOperation::FlowCoalesceLatest(
        FlowPressureOperation,
    ))
}

fn validate_backpressure(placement: &PlannedGear) -> Result<(), String> {
    validate(
        placement,
        &conduit_std_offers::flow_backpressure_std_offer(
            &placement
                .inputs
                .first()
                .ok_or_else(|| "planned flow/backpressure input missing".to_string())?
                .value_kind,
            placement.limits.max_queue_bytes,
        ),
        PortTemporal::Flow { closes: false },
    )
}

fn validate_coalesce(placement: &PlannedGear) -> Result<(), String> {
    validate(
        placement,
        &conduit_std_offers::flow_coalesce_latest_std_offer(
            &placement
                .inputs
                .first()
                .ok_or_else(|| "planned flow/coalesce-latest input missing".to_string())?
                .value_kind,
            placement.limits.max_queue_bytes,
        ),
        PortTemporal::Current,
    )
}

fn validate(
    placement: &PlannedGear,
    offer: &conduit_core::CapabilityOffer,
    output_temporal: PortTemporal,
) -> Result<(), String> {
    let [input] = placement.inputs.as_slice() else {
        return Err("flow pressure operation requires one input".into());
    };
    let [output] = placement.outputs.as_slice() else {
        return Err("flow pressure operation requires one output".into());
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
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.configuration.is_empty()
        || input.direction != PortDirection::Input
        || input.temporal != (PortTemporal::Flow { closes: false })
        || output.direction != PortDirection::Output
        || output.temporal != output_temporal
        || input.value_kind != output.value_kind
    {
        return Err("planned flow pressure operation differs from its installation".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, SCALAR_ENCODED_LEN, SCALAR_INFO_ID};
    use conduit_kernel::{OperationAction, ValueRef};

    #[test]
    fn flow_pressure_operation_passes_values_and_closure() {
        let mut operation = FlowPressureOperation;
        let value = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: SCALAR_ENCODED_LEN as u32,
        };
        assert_eq!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value,
            }),
            OperationAction::Emit {
                port: PortId(0),
                value,
            }
        );
        assert_eq!(operation.advance(), OperationAction::Await);
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Complete
        );
        let offer = conduit_std_offers::flow_coalesce_latest_std_offer(
            &kind_id(SCALAR_INFO_ID),
            SCALAR_ENCODED_LEN as u32,
        );
        assert_eq!(offer.outputs[0].temporal, PortTemporal::Current);
    }
}
