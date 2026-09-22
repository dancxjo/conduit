//! Native installations for the shared semantic mapper and honest stdout sink.
use crate::installed_std::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{CapabilityOffer, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, Failure, FailureCode, HostedValueStore, PortId,
};
use conduit_semantic_catalog::{PreparedButtonIndicatorMapper, BUTTON_TRANSITION_MAXIMUM_VALUES};

pub(in crate::installed_std) static MAPPER: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::button::MAPPER,
    budget: mapper_budget,
    prepare: prepare_mapper,
};
pub(in crate::installed_std) static INDICATOR: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::button::INDICATOR,
    budget: indicator_budget,
    prepare: prepare_indicator,
};
pub(in crate::installed_std) static RESOURCE_INDICATOR: BackFactory = BackFactory {
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

impl<const PORTS: usize> StepBack<PORTS> for Mapper {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            if self.closed || self.emitted == BUTTON_TRANSITION_MAXIMUM_VALUES as usize {
                return mapper_step_fail(63);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(bytes) = input_bytes.input(PortId(0)) else {
                return mapper_step_fail(62);
            };
            let Ok(state) = self.mapper.map(bytes) else {
                return mapper_step_fail(62);
            };
            io.consume(PortId(0)).expect("present button transition");
            io.send_canonical(
                PortId(0),
                CanonicalValue::new(&state.encode()).expect("one bounded Boolean byte"),
            )
            .expect("ready button-indicator output");
            self.emitted += 1;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && !self.closed {
            io.consume_closed(PortId(0))
                .expect("observed button-indicator closure");
            self.closed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

const fn mapper_step_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

impl Mapper {}

fn validate(placement: &PlannedGear, offer: CapabilityOffer) -> Result<(), String> {
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

fn mapper_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement, conduit_std_offers::button::mapper_offer())?;
    Ok(BackBudget {
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
) -> Result<InstalledBack, String> {
    validate(placement, conduit_std_offers::button::mapper_offer())?;
    Ok(InstalledBack::ButtonMapper(Box::new(Mapper {
        mapper: PreparedButtonIndicatorMapper::new().map_err(|error| format!("{error:?}"))?,
        emitted: 0,
        closed: false,
    })))
}

fn indicator_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement, indicator_offer(placement))?;
    Ok(BackBudget {
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
) -> Result<InstalledBack, String> {
    validate(placement, indicator_offer(placement))?;
    Ok(InstalledBack::BoolPresentation(
        crate::installed_std::bool_presentation::BoolPresentationBack::new(u64::from(
            BUTTON_TRANSITION_MAXIMUM_VALUES,
        )),
    ))
}
