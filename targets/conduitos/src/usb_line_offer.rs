//! Boot-scoped Line truth derived from one exact QEMU FTDI realization.

use conduit_core::{
    BaseImplementationId, BaseInstanceId, BootId, HostId, LineAvailability, LineAvailabilitySign,
    LineContinuation, LineContract, LineDuplex, LineId, LineOffer, LineOrdering, LineReliability,
    LineScope, LineSecurity, LineTrafficShape, LinkAuthorityReference, LinkBinding, LinkBindingId,
    LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits, SignId,
};

pub const USB_FTDI_BASE: &str = "conduit.base/qemu-usb-ftdi@1";
pub const USB_LINE_MAXIMUM_IN_FLIGHT_ITEMS: u16 = 1;
pub const USB_LINE_MAXIMUM_PAYLOAD_BYTES: u32 = 64;
pub const USB_LINE_MAXIMUM_BUFFERED_BYTES: u32 = 1_024;
pub const USB_LINE_MAXIMUM_FRAME_BYTES: u32 = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsbLineRealization {
    pub controller_id: [u8; 32],
    pub device_id: [u8; 32],
    pub interface_id: [u8; 32],
    pub input_endpoint_id: [u8; 32],
    pub output_endpoint_id: [u8; 32],
    pub attachment_epoch: u32,
    pub input_dci: u8,
    pub output_dci: u8,
    pub packet_bytes: u16,
    pub payload_bytes: u16,
    pub transfer_trbs_per_direction: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsbLineState {
    Current,
    Lost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsbLineObservation {
    pub realization: UsbLineRealization,
    pub base_instance_id: BaseInstanceId,
    pub state: UsbLineState,
    pub state_sign_id: SignId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsbLineIdentity {
    pub line_id: LineId,
    pub binding_id: LinkBindingId,
    pub base_instance_id: BaseInstanceId,
    pub source_host_id: HostId,
    pub source_boot_id: BootId,
    pub source_endpoint_id: LinkEndpointId,
    pub sink_host_id: HostId,
    pub sink_boot_id: BootId,
    pub sink_endpoint_id: LinkEndpointId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedUsbLineBasis {
    pub line: conduit_core::AdmittedLine,
    realization: UsbLineRealization,
    state_sign_id: SignId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsbLineOfferError {
    EmptyIdentity,
    DuplicateDeviceIdentity,
    InvalidCapacity,
    WrongEndpoint,
    NotCurrent,
    BaseInstanceMismatch,
    RealizationMismatch,
    StaleState,
    Lost,
}

impl UsbLineRealization {
    pub fn validate(self) -> Result<(), UsbLineOfferError> {
        let identities = [
            self.controller_id,
            self.device_id,
            self.interface_id,
            self.input_endpoint_id,
            self.output_endpoint_id,
        ];
        if identities.contains(&[0; 32]) || self.attachment_epoch == 0 {
            return Err(UsbLineOfferError::EmptyIdentity);
        }
        if identities
            .iter()
            .enumerate()
            .any(|(index, identity)| identities[..index].contains(identity))
        {
            return Err(UsbLineOfferError::DuplicateDeviceIdentity);
        }
        if self.input_dci != 3 || self.output_dci != 4 {
            return Err(UsbLineOfferError::WrongEndpoint);
        }
        if self.packet_bytes != crate::arch::FTDI_PACKET_BYTES as u16
            || self.payload_bytes != crate::arch::FTDI_PAYLOAD_BYTES as u16
            || self.transfer_trbs_per_direction != crate::arch::FTDI_TRANSFER_TRBS as u8
        {
            return Err(UsbLineOfferError::InvalidCapacity);
        }
        Ok(())
    }
}

pub fn offer_usb_ftdi_line(
    identity: UsbLineIdentity,
    observation: &UsbLineObservation,
) -> Result<LineOffer, UsbLineOfferError> {
    observation.realization.validate()?;
    if observation.state != UsbLineState::Current {
        return Err(UsbLineOfferError::NotCurrent);
    }
    if observation.base_instance_id != identity.base_instance_id {
        return Err(UsbLineOfferError::BaseInstanceMismatch);
    }
    let required = [
        identity.line_id.as_str(),
        identity.binding_id.as_str(),
        identity.base_instance_id.as_str(),
        identity.source_host_id.as_str(),
        identity.source_boot_id.as_str(),
        identity.source_endpoint_id.as_str(),
        identity.sink_host_id.as_str(),
        identity.sink_boot_id.as_str(),
        identity.sink_endpoint_id.as_str(),
        observation.state_sign_id.as_str(),
    ];
    if required.contains(&"") || identity.source_host_id == identity.sink_host_id {
        return Err(UsbLineOfferError::EmptyIdentity);
    }
    let line_id = identity.line_id.clone();
    let binding_id = identity.binding_id.clone();
    Ok(LineOffer {
        line_id: identity.line_id,
        binding: LinkBinding {
            binding_id: identity.binding_id,
            source: LinkEndpoint {
                host_id: identity.source_host_id,
                boot_id: identity.source_boot_id,
                endpoint_id: identity.source_endpoint_id,
            },
            sink: LinkEndpoint {
                host_id: identity.sink_host_id,
                boot_id: identity.sink_boot_id,
                endpoint_id: identity.sink_endpoint_id,
            },
            base: BaseImplementationId::from(USB_FTDI_BASE),
            base_instance_id: identity.base_instance_id,
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: usb_ftdi_limits(),
        },
        contract: usb_ftdi_contract(),
        availability: LineAvailabilitySign {
            line_id,
            binding_id,
            availability: LineAvailability::Ready,
            sign_id: observation.state_sign_id.clone(),
        },
    })
}

impl AdmittedUsbLineBasis {
    pub fn from_offer(
        offer: &LineOffer,
        observation: &UsbLineObservation,
    ) -> Result<Self, UsbLineOfferError> {
        if offer.binding.base != BaseImplementationId::from(USB_FTDI_BASE)
            || offer.binding.base_instance_id != observation.base_instance_id
        {
            return Err(UsbLineOfferError::BaseInstanceMismatch);
        }
        if offer.contract != usb_ftdi_contract()
            || offer.binding.limits != usb_ftdi_limits()
            || !offer.validate_sign_identity()
            || offer.availability.availability != LineAvailability::Ready
        {
            return Err(UsbLineOfferError::RealizationMismatch);
        }
        observation.realization.validate()?;
        if observation.state != UsbLineState::Current {
            return Err(UsbLineOfferError::NotCurrent);
        }
        if offer.availability.sign_id != observation.state_sign_id {
            return Err(UsbLineOfferError::StaleState);
        }
        Ok(Self {
            line: offer.admitted_line(),
            realization: observation.realization,
            state_sign_id: observation.state_sign_id.clone(),
        })
    }

    pub fn validate_current(
        &self,
        observation: &UsbLineObservation,
    ) -> Result<(), UsbLineOfferError> {
        if observation.state == UsbLineState::Lost {
            return Err(UsbLineOfferError::Lost);
        }
        observation.realization.validate()?;
        if observation.base_instance_id != self.line.binding.base_instance_id {
            return Err(UsbLineOfferError::BaseInstanceMismatch);
        }
        if observation.realization != self.realization {
            return Err(UsbLineOfferError::RealizationMismatch);
        }
        if observation.state_sign_id != self.state_sign_id {
            return Err(UsbLineOfferError::StaleState);
        }
        Ok(())
    }
}

pub const fn usb_ftdi_contract() -> LineContract {
    LineContract {
        scope: LineScope::PointToPoint,
        traffic_shape: LineTrafficShape::ByteStream,
        duplex: LineDuplex::FullDuplex,
        ordering: LineOrdering::Ordered,
        reliability: LineReliability::Reliable,
        continuation: LineContinuation::None,
        security: LineSecurity::ProcessBoundary,
    }
}

pub const fn usb_ftdi_limits() -> LinkLimits {
    LinkLimits {
        maximum_in_flight_items: USB_LINE_MAXIMUM_IN_FLIGHT_ITEMS,
        maximum_payload_bytes: USB_LINE_MAXIMUM_PAYLOAD_BYTES,
        maximum_buffered_bytes: USB_LINE_MAXIMUM_BUFFERED_BYTES,
        maximum_frame_bytes: USB_LINE_MAXIMUM_FRAME_BYTES,
    }
}

#[cfg(test)]
#[path = "usb_line_offer_tests.rs"]
mod tests;
