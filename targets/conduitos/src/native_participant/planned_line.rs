//! Prepare one exact planned Cord on the existing finite FTDI Line profile.
//! This validates realization, not membership, authority, or fragment execution.
pub use super::usb_carrier::UsbCarrier;
use crate::{
    arch::FtdiLineReady,
    usb_line_offer::{AdmittedUsbLineBasis, UsbLineObservation},
    usb_line_session::UsbLineSession,
};
use conduit_core::{HostAdvertisement, Plan};
use conduit_wire::{SessionBinding, SessionRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedLineError {
    Plan,
    Binding,
    Endpoint,
    StaleHost,
    SelectedLine,
    Observation(crate::usb_line_offer::UsbLineOfferError),
    Carrier,
}

pub struct PlannedUsbLine {
    basis: AdmittedUsbLineBasis,
    observation: UsbLineObservation,
    session: UsbLineSession,
}
impl PlannedUsbLine {
    pub fn admit(
        plan: &Plan,
        host: &HostAdvertisement,
        binding: &SessionBinding,
        basis: &AdmittedUsbLineBasis,
        observation: &UsbLineObservation,
        role: SessionRole,
    ) -> Result<Self, PlannedLineError> {
        if !conduit_core::verify_plan(plan) || binding.plan_id != plan.plan_id {
            return Err(PlannedLineError::Plan);
        }
        let source = plan
            .fragments
            .iter()
            .find(|p| p.fragment_id == binding.source_fragment_id)
            .ok_or(PlannedLineError::Endpoint)?;
        let sink = plan
            .fragments
            .iter()
            .find(|p| p.fragment_id == binding.sink_fragment_id)
            .ok_or(PlannedLineError::Endpoint)?;
        if source.host_id != binding.source.host_id
            || source.boot_id != binding.source.boot_id
            || sink.host_id != binding.sink.host_id
            || sink.boot_id != binding.sink.boot_id
        {
            return Err(PlannedLineError::Endpoint);
        }
        let local = match role {
            SessionRole::Source => source,
            SessionRole::Sink => sink,
        };
        if local.host_id != host.host_id
            || local.boot_id != host.boot_id
            || local.offer_generation != host.offer_generation
        {
            return Err(PlannedLineError::StaleHost);
        }
        let mut connections = source
            .connections
            .iter()
            .filter(|c| c.connection_id == binding.connection_id);
        let connection = connections.next().ok_or(PlannedLineError::Binding)?;
        if connections.next().is_some()
            || sink
                .connections
                .iter()
                .filter(|c| c.connection_id == binding.connection_id)
                .count()
                != 1
            || !sink.connections.iter().any(|c| c == connection)
        {
            return Err(PlannedLineError::Binding);
        }
        if !source.placements.iter().any(|p| {
            p.placement_id == connection.source_placement_id
                && p.host_id == source.host_id
                && p.boot_id == source.boot_id
                && p.outputs.iter().any(|port| {
                    port.port_id == connection.source_port_id
                        && port.value_kind == connection.value_kind
                        && port.temporal == connection.temporal
                })
        }) || !sink.placements.iter().any(|p| {
            p.placement_id == connection.sink_placement_id
                && p.host_id == sink.host_id
                && p.boot_id == sink.boot_id
                && p.inputs.iter().any(|port| {
                    port.port_id == connection.sink_port_id
                        && port.value_kind == connection.value_kind
                        && port.temporal == connection.temporal
                })
        }) {
            return Err(PlannedLineError::Endpoint);
        }
        if connection.selected_line.as_ref() != Some(&basis.line) {
            return Err(PlannedLineError::SelectedLine);
        }
        let exact = SessionBinding::from_planned_connection(
            plan.plan_id.clone(),
            source.fragment_id.clone(),
            sink.fragment_id.clone(),
            connection,
        )
        .map_err(|_| PlannedLineError::Binding)?;
        if exact != *binding {
            return Err(PlannedLineError::Binding);
        }
        basis
            .validate_current(observation)
            .map_err(PlannedLineError::Observation)?;
        super::usb_carrier::validate_binding(binding, basis)?;
        let session =
            UsbLineSession::new(binding.clone(), role).map_err(|_| PlannedLineError::Binding)?;
        Ok(Self {
            basis: basis.clone(),
            observation: observation.clone(),
            session,
        })
    }
    pub fn binding(&self) -> &SessionBinding {
        self.session.binding()
    }
    pub fn attach(self, ready: FtdiLineReady) -> Result<UsbCarrier, PlannedLineError> {
        UsbCarrier::attach(self.basis, self.observation, self.session, ready)
    }
}
#[cfg(test)]
#[path = "planned_line_tests.rs"]
mod tests;
