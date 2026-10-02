//! Physical carrier shared by planned participants and the named connectivity harness.
use super::planned_line::PlannedLineError;
use crate::{
    arch::{FtdiLineReady, FtdiLineSession, UsbDevice, XhciReady, start_ftdi_line_session},
    usb_line_offer::{
        AdmittedUsbLineBasis, USB_FTDI_BASE, UsbLineObservation, UsbLineState, usb_ftdi_contract,
        usb_ftdi_limits,
    },
    usb_line_session::{ReceivedSessionMessage, UsbLineSession, UsbLineSessionError},
};
use conduit_wire::{SessionBinding, SessionMessage, SessionRole};

pub struct UsbCarrier {
    pub(crate) observation: UsbLineObservation,
    pub(crate) basis: AdmittedUsbLineBasis,
    pub(crate) carrier: FtdiLineSession,
    pub(crate) session: UsbLineSession,
}
pub(super) fn validate_binding(
    binding: &SessionBinding,
    basis: &AdmittedUsbLineBasis,
) -> Result<(), PlannedLineError> {
    let line = &basis.line;
    if line.binding.base.as_str() != USB_FTDI_BASE
        || line.contract != usb_ftdi_contract()
        || line.binding.limits != usb_ftdi_limits()
    {
        return Err(PlannedLineError::SelectedLine);
    }
    let exact = &binding.attachment;
    if exact.line_id != line.line_id
        || exact.link_binding_id != line.binding.binding_id
        || exact.base != line.binding.base
        || exact.base_instance_id != line.binding.base_instance_id
        || exact.contract != line.contract
        || exact.limits != line.binding.limits
        || exact.source_host_id != line.binding.source.host_id
        || exact.source_boot_id != line.binding.source.boot_id
        || exact.source_endpoint_id != line.binding.source.endpoint_id
        || exact.sink_host_id != line.binding.sink.host_id
        || exact.sink_boot_id != line.binding.sink.boot_id
        || exact.sink_endpoint_id != line.binding.sink.endpoint_id
    {
        return Err(PlannedLineError::SelectedLine);
    }
    Ok(())
}
impl UsbCarrier {
    pub(super) fn attach(
        basis: AdmittedUsbLineBasis,
        observation: UsbLineObservation,
        session: UsbLineSession,
        ready: FtdiLineReady,
    ) -> Result<Self, PlannedLineError> {
        basis
            .validate_current(&observation)
            .map_err(PlannedLineError::Observation)?;
        validate_binding(session.binding(), &basis)?;
        let physical = observation.realization;
        if crate::identity::derive_usb_interface(&physical.device_id, ready.interface_number, 0)
            != physical.interface_id
            || crate::identity::derive_usb_endpoint(
                &physical.interface_id,
                ready.input_endpoint_address,
            ) != physical.input_endpoint_id
            || crate::identity::derive_usb_endpoint(
                &physical.interface_id,
                ready.output_endpoint_address,
            ) != physical.output_endpoint_id
            || ready.input_dci != physical.input_dci
            || ready.output_dci != physical.output_dci
            || ready.packet_bytes != physical.packet_bytes
            || ready.payload_bytes != physical.payload_bytes
            || ready.transfer_trbs_per_direction != physical.transfer_trbs_per_direction
        {
            return Err(PlannedLineError::Carrier);
        }
        Ok(Self {
            basis,
            observation,
            session,
            carrier: start_ftdi_line_session(ready),
        })
    }
    /// Connectivity proof only: no sealed Plan or membership is claimed here.
    pub(crate) fn for_connectivity_harness(
        binding: SessionBinding,
        basis: AdmittedUsbLineBasis,
        observation: UsbLineObservation,
        role: SessionRole,
        ready: FtdiLineReady,
    ) -> Result<Self, PlannedLineError> {
        let session = UsbLineSession::new(binding, role).map_err(|_| PlannedLineError::Binding)?;
        Self::attach(basis, observation, session, ready)
    }
    pub fn binding(&self) -> &SessionBinding {
        self.session.binding()
    }
    pub fn send(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
        message: SessionMessage<'_>,
    ) -> Result<(), UsbLineSessionError> {
        self.session.send(
            &mut self.carrier,
            controller,
            device,
            &self.basis,
            &self.observation,
            message,
        )
    }
    pub fn receive(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
    ) -> Result<ReceivedSessionMessage, UsbLineSessionError> {
        self.session.receive(
            &mut self.carrier,
            controller,
            device,
            &self.basis,
            &self.observation,
        )
    }
    pub fn observe(&mut self, observation: UsbLineObservation) -> Result<(), UsbLineSessionError> {
        if self.observation.state == UsbLineState::Lost {
            return Err(UsbLineSessionError::Current(
                crate::usb_line_offer::UsbLineOfferError::Lost,
            ));
        }
        if let Err(error) = self.basis.validate_current(&observation) {
            self.observation.state = UsbLineState::Lost;
            return Err(UsbLineSessionError::Current(error));
        }
        self.observation = observation;
        Ok(())
    }
}
