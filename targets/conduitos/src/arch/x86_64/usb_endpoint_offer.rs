//! Root preparation for a configured class-neutral inbound endpoint Back.
#![allow(dead_code)] // Ordinary HID composition is being connected separately.
use super::endpoint_setup::ConfiguredInboundEndpoint;
use super::{UsbDevice, dma::device_dma_pointer, endpoint_read::EndpointReceiveDma};
use crate::usb_base::{
    endpoint_read_contract::EndpointReadContract,
    endpoint_read_factory::{ENDPOINT_READ_ATTACHMENT, ENDPOINT_READ_BASE},
    endpoint_read_offer,
};
use alloc::{format, vec};
use conduit_core::*;

/// A Configure Endpoint receipt and still-retained DMA establish readiness.
/// This description issues no authority or executable Base possession.
pub(super) fn ready_entry(
    controller_base_id: &str,
    device_instance_id: &str,
    device: UsbDevice,
    configured: &ConfiguredInboundEndpoint,
    dma: &EndpointReceiveDma<'_>,
    contract: &EndpointReadContract,
) -> Result<BaseProviderEntry, &'static str> {
    if configured.slot != device.slot
        || configured.device_epoch != device.attachment_epoch
        || configured.endpoint_epoch == 0
        || configured.dci < 3
        || configured.dci > 31
        || configured.dci & 1 == 0
        || configured.ring_physical != dma.ring_physical
        || dma.ring_physical == 0
        || dma.buffer_physical == 0
        || dma.cursor.ordinary_slots() != 63
        || device.root_port == 0
        || device.attachment_epoch == 0
    {
        return Err("usb-endpoint-offer-binding");
    }
    device_dma_pointer(&device).map_err(|_| "usb-endpoint-offer-device")?;
    dma.cursor
        .ensure_idle()
        .map_err(|_| "usb-endpoint-offer-active")?;
    Ok(BaseProviderEntry {
        base_id: controller_base_id.into(),
        provider_instance_id: device_instance_id.into(),
        provider_generation: u64::from(device.attachment_epoch),
        implementation_id: ENDPOINT_READ_BASE.into(),
        mechanism_family: ENDPOINT_READ_ATTACHMENT.into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![endpoint_read_offer::offer(
            contract,
            "conduitos/usb-endpoint-read-native@1".into(),
        )],
        resources: vec![resource_offer(
            &format!(
                "usb-endpoint-read-dma/{}/{}/{}/{}/{}",
                device.root_port,
                device.slot,
                device.attachment_epoch,
                configured.dci,
                configured.endpoint_epoch
            ),
            ENDPOINT_READ_ATTACHMENT,
            1,
        )],
    })
}
