//! Native installations for the shared semantic mapper and honest stdout sink.
use crate::installed_std::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{CapabilityOffer, PlannedGear};
use conduit_kernel::{CanonicalValue, HostedValueStore, OperationAction, OperationInput, PortId};
use conduit_semantic_catalog::{PreparedButtonIndicatorMapper, BUTTON_TRANSITION_MAXIMUM_VALUES};

pub(in crate::installed_std) static MAPPER: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::button::MAPPER,
    budget: mapper_budget,
    prepare: prepare_mapper,
};
pub(in crate::installed_std) static INDICATOR: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::button::INDICATOR,
    budget: indicator_budget,
    prepare: prepare_indicator,
};
pub(in crate::installed_std) static RESOURCE_INDICATOR: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::indicator_resource::IMPLEMENTATION,
    budget: indicator_budget,
    prepare: prepare_indicator,
};

fn indicator_offer(placement: &PlannedGear) -> CapabilityOffer {
    if placement.implementation_id.as_str()
        == conduit_std_offers::indicator_resource::IMPLEMENTATION
    {
        conduit_std_offers::indicator_resource::offer()
    } else {
        conduit_std_offers::button::indicator_offer()
    }
}

pub(in crate::installed_std) struct Mapper {
    mapper: PreparedButtonIndicatorMapper,
    emitted: usize,
    closed: bool,
}

impl Mapper {
    pub(in crate::installed_std) fn resume_value(
        &mut self,
        port: PortId,
        bytes: &[u8],
    ) -> OperationAction {
        if port != PortId(0) {
            return InstalledOperation::fail(61);
        }
        let Ok(state) = self.mapper.map(bytes) else {
            return InstalledOperation::fail(62);
        };
        if self.closed || self.emitted == BUTTON_TRANSITION_MAXIMUM_VALUES as usize {
            return InstalledOperation::fail(63);
        }
        self.emitted += 1;
        OperationAction::EmitCanonical {
            port: PortId(0),
            value: CanonicalValue::new(&state.encode()).expect("one bounded Boolean byte"),
        }
    }
    pub(in crate::installed_std) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port: PortId(0) } => {
                self.closed = true;
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(60),
        }
    }
}

fn validate(placement: &PlannedGear, offer: CapabilityOffer) -> Result<(), String> {
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
        || !placement.authority.is_empty()
        || placement.resources.len() != offer.resource_requirements.len()
        || !offer.resource_requirements.iter().all(|required| {
            placement.resources.iter().any(|resource| {
                resource.class_id == required.class_id
                    && resource.units == required.units
                    && resource.protected.is_none()
                    && resource.compute.is_none()
            })
        })
    {
        return Err("planned button-indicator installation mismatch".into());
    }
    Ok(())
}

fn mapper_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, conduit_std_offers::button::mapper_offer())?;
    Ok(OperationBudget {
        value_items: BUTTON_TRANSITION_MAXIMUM_VALUES * 2,
        value_bytes: u32::from(BUTTON_TRANSITION_MAXIMUM_VALUES) * 2,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
    })
}

fn prepare_mapper(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, conduit_std_offers::button::mapper_offer())?;
    Ok(InstalledOperation::ButtonMapper(Box::new(Mapper {
        mapper: PreparedButtonIndicatorMapper::new().map_err(|error| format!("{error:?}"))?,
        emitted: 0,
        closed: false,
    })))
}

fn indicator_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, indicator_offer(placement))?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: BUTTON_TRANSITION_MAXIMUM_VALUES as usize,
        sign_items: 64,
        maximum_value_bytes: 1,
    })
}

fn prepare_indicator(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, indicator_offer(placement))?;
    Ok(InstalledOperation::BoolPresentation(
        crate::installed_std::bool_presentation::BoolPresentationOperation::new(u64::from(
            BUTTON_TRANSITION_MAXIMUM_VALUES,
        )),
    ))
}
