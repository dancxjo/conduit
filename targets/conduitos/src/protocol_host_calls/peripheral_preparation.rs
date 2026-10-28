//! Bind each retained peripheral owner to exactly one selected ordinary node.
use super::*;
use crate::{
    digital_base::{
        contract::DigitalContract, installation::DIGITAL_IMPLEMENTATION,
        owner::DigitalCallSelection,
    },
    protocol_play::PeripheralAdmission,
    spi_base::{contract::SpiContract, installation::SPI_IMPLEMENTATION, owner::SpiCallSelection},
};
impl<P: I2cProvider> PreparedProtocolCalls<P> {
    pub(super) fn revoke_peripherals(&mut self) -> Result<(), ProtocolCallRefusal> {
        let mut failure = None;
        for binding in &mut self.bindings {
            let result = match &mut binding.owner {
                Owner::Spi(owner) => owner
                    .revoke(binding.node, HostCallId(0))
                    .map_err(ProtocolCallRefusal::Spi),
                Owner::Digital(owner) => owner
                    .revoke(binding.node, HostCallId(0))
                    .map_err(ProtocolCallRefusal::Digital),
                _ => Ok(()),
            };
            if let Err(error) = result {
                if failure.is_none() {
                    failure = Some(error);
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }

    pub(crate) fn prepare_peripheral(
        plan: &Plan,
        admission: PeripheralAdmission,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        if admission.digital.len() > 64 || (admission.digital.is_empty() && admission.spi.is_none())
        {
            return Err(ProtocolCallRefusal::Unsupported);
        }
        let mut implementations = Vec::with_capacity(2);
        if admission.spi.is_some() {
            implementations.push(SPI_IMPLEMENTATION);
        }
        if !admission.digital.is_empty() {
            implementations.push(DIGITAL_IMPLEMENTATION);
        }
        let (lowered, active, mut bindings) =
            Self::prepare_pure(plan, &implementations, zip, states)?;
        let fragment = &plan.fragments[0];
        let count = |implementation| {
            fragment
                .placements
                .iter()
                .filter(|gear| gear.implementation_id.as_str() == implementation)
                .count()
        };
        if count(DIGITAL_IMPLEMENTATION) != admission.digital.len() {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        if let Some(spi) = admission.spi {
            let gear = fragment
                .placements
                .iter()
                .find(|gear| {
                    gear.placement_id == spi.placement
                        && gear.implementation_id.as_str() == SPI_IMPLEMENTATION
                })
                .ok_or(ProtocolCallRefusal::InvalidPlan)?;
            let contract = SpiContract::prepare().map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
            let owner = spi
                .ready
                .bind_selected(
                    spi.table,
                    spi.handle,
                    spi.claim,
                    SpiCallSelection {
                        contract: &contract,
                        fragment,
                        lowered: &lowered,
                        active: &active,
                        placement: &gear.placement_id,
                    },
                )
                .map_err(ProtocolCallRefusal::Spi)?;
            bindings.push(Binding {
                node: lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or(ProtocolCallRefusal::InvalidPlan)?,
                resources: gear.resources.clone(),
                authorities: gear.authority.clone(),
                owner: Owner::Spi(Box::new(owner)),
            });
        }
        let contract = DigitalContract::prepare().map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        for digital in admission.digital {
            let gear = fragment
                .placements
                .iter()
                .find(|gear| {
                    gear.placement_id == digital.placement
                        && gear.implementation_id.as_str() == DIGITAL_IMPLEMENTATION
                })
                .ok_or(ProtocolCallRefusal::InvalidPlan)?;
            let node = lowered
                .identity
                .node_for_placement(&gear.placement_id)
                .ok_or(ProtocolCallRefusal::InvalidPlan)?;
            if bindings.iter().any(|binding| binding.node == node) {
                return Err(ProtocolCallRefusal::InvalidPlan);
            }
            let owner = digital
                .ready
                .bind_selected(
                    digital.table,
                    digital.handle,
                    digital.claim,
                    DigitalCallSelection {
                        contract: &contract,
                        fragment,
                        lowered: &lowered,
                        active: &active,
                        placement: &gear.placement_id,
                    },
                )
                .map_err(ProtocolCallRefusal::Digital)?;
            bindings.push(Binding {
                node,
                resources: gear.resources.clone(),
                authorities: gear.authority.clone(),
                owner: Owner::Digital(Box::new(owner)),
            });
        }
        Ok(Self::from_validated_bindings(plan, bindings))
    }
}
