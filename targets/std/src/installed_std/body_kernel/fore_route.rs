//! Exact, finite local Fore boundary for one installed Body Play.
use crate::{ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter};
use conduit_core::PortDirection;
use conduit_plan_lowering::lowering::{LoweredForePort, LoweredPlanFragment};

use super::super::{
    external_fore_feeder::{self, PreparedForeInputs},
    InstalledScheduler,
};

pub(super) struct BodyForeRoute<'a> {
    inputs: PreparedForeInputs<'a>,
    ports: Vec<LoweredForePort>,
    deliveries: Vec<ExternalForeDelivery>,
    started: bool,
}

impl<'a> BodyForeRoute<'a> {
    pub(super) fn prepare(
        partitions: &[LoweredPlanFragment],
        supplied: &'a [ExternalForeInput],
        sequential: bool,
        has_output: bool,
    ) -> Result<Self, String> {
        let planned_inputs = partitions
            .iter()
            .flat_map(|part| &part.fore_ports)
            .filter(|port| port.direction == PortDirection::Input)
            .collect::<Vec<_>>();
        let inputs = PreparedForeInputs::prepare(&planned_inputs, supplied, sequential)?;
        // An external Fore input or delivery has no Plot discriminator. Refuse
        // names shared by distinct resident Plots instead of inventing fan-out.
        let mut seen = Vec::new();
        for (index, part) in partitions.iter().enumerate() {
            for port in &part.fore_ports {
                if seen
                    .iter()
                    .any(|(prior, earlier): &(usize, &LoweredForePort)| {
                        *prior != index
                            && earlier.front_port_id == port.front_port_id
                            && earlier.direction == port.direction
                            && earlier.track == port.track
                    })
                {
                    return Err("Body Fore port identity is ambiguous across Plots".into());
                }
                seen.push((index, port));
            }
        }
        let ports = partitions
            .iter()
            .flat_map(|part| &part.fore_ports)
            .cloned()
            .collect::<Vec<_>>();
        let mut outputs = ports
            .iter()
            .filter(|port| port.direction == PortDirection::Output);
        if !has_output && outputs.clone().next().is_some() {
            return Err("sealed Body Fore output has no acknowledging adapter".into());
        }
        let delivery_capacity = outputs.try_fold(0_usize, |sum, port| {
            sum.checked_add(usize::from(port.item_capacity.max(inputs.input_items())))
                .ok_or("Body Fore delivery capacity overflow")
        })?;
        Ok(Self {
            inputs,
            ports,
            deliveries: Vec::with_capacity(delivery_capacity),
            started: false,
        })
    }

    pub(super) fn input_ports(&self) -> impl Iterator<Item = &LoweredForePort> {
        self.ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input)
    }

    pub(super) fn sign_items(&self) -> Result<u16, String> {
        external_fore_feeder::remote_sign_items(&self.ports, self.inputs.input_items())
    }

    pub(super) fn has_ports(&self) -> bool {
        !self.ports.is_empty()
    }

    pub(super) fn start_and_feed(
        &mut self,
        scheduler: &mut InstalledScheduler,
    ) -> Result<(), String> {
        if !self.started {
            // Ingress lifecycle Signs belong to the admitted Body Play.
            self.inputs.start(scheduler)?;
            self.started = true;
        }
        self.inputs.feed_next(scheduler)
    }

    pub(super) fn drain(
        &mut self,
        scheduler: &mut InstalledScheduler,
        output: &mut Option<&mut dyn ExternalForeOutputAdapter>,
    ) -> Result<(), String> {
        for planned in self
            .ports
            .iter()
            .filter(|port| port.direction == PortDirection::Output)
        {
            while let Some(offer) = scheduler
                .remote_egress_offer(planned.endpoint, planned.cord)
                .map_err(|error| format!("Body Fore offer: {error:?}"))?
            {
                if self.deliveries.len() == self.deliveries.capacity() {
                    return Err("Body Fore delivery capacity exceeded".into());
                }
                let bytes = scheduler
                    .host_value(offer.value)
                    .map_err(|error| format!("Body Fore value: {error:?}"))?
                    .to_vec();
                let delivery = ExternalForeDelivery {
                    front_port_id: planned.front_port_id.clone(),
                    track: planned.track,
                    value_kind: planned.value_kind.clone(),
                    sequence: offer.sequence,
                    bytes,
                };
                output
                    .as_deref_mut()
                    .ok_or("Body Fore output adapter lost")?
                    .deliver(delivery.clone())?;
                scheduler
                    .remote_egress_accept(planned.endpoint, planned.cord, offer.sequence)
                    .map_err(|error| format!("Body Fore accept: {error:?}"))?;
                scheduler
                    .remote_egress_delivered(planned.endpoint, planned.cord, offer.sequence)
                    .map_err(|error| format!("Body Fore acknowledge: {error:?}"))?;
                self.deliveries.push(delivery);
            }
        }
        Ok(())
    }

    pub(super) fn require_normal_terminal(
        &self,
        scheduler: &InstalledScheduler,
    ) -> Result<(), String> {
        for planned in self
            .ports
            .iter()
            .filter(|port| port.direction == PortDirection::Output)
        {
            if !scheduler
                .remote_egress_terminal(planned.endpoint, planned.cord)
                .map_err(|error| format!("Body Fore terminal: {error:?}"))?
            {
                return Err(format!(
                    "Body Fore output '{}' did not reach an observed normal terminal",
                    planned.front_port_id.as_str(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn into_deliveries(self) -> Vec<ExternalForeDelivery> {
        self.deliveries
    }
}
