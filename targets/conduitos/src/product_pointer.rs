//! Prepared USB pointer realization and ordinary Tour interaction service.

use alloc::format;

use conduit_tour_model::TourPointerOutcome;

use crate::{
    arch::{self, HidPointerReady, HidPointerSession, UsbDevice, XhciReady},
    fabrication::FabricationRecord,
    identity::{self, BootIdentities},
    pointer_offer::PointerRealization,
    tour_product::TourProduct,
};

pub fn realization(
    identities: &BootIdentities,
    controller_id: [u8; 32],
    device: &UsbDevice,
    ready: HidPointerReady,
) -> Result<PointerRealization, &'static str> {
    let device_id = identity::derive_usb_device(
        &identities.boot,
        &controller_id,
        device.root_port,
        device.slot,
        device.attachment_epoch,
    );
    let interface = device.interfaces[..usize::from(device.interface_count)]
        .iter()
        .find(|interface| {
            interface.number == ready.interface_number && interface.alternate_setting == 0
        })
        .ok_or("pointer-interface-identity-absent")?;
    let interface_id =
        identity::derive_usb_interface(&device_id, interface.number, interface.alternate_setting);
    let endpoint_id = identity::derive_usb_endpoint(&interface_id, ready.endpoint_address);
    Ok(PointerRealization {
        controller_id,
        device_id,
        interface_id,
        endpoint_id,
        report_buffers: u16::from(ready.report_buffers),
        event_slots: crate::pointer_offer::POINTER_EVENT_SLOTS,
        operation_slots: crate::pointer_offer::POINTER_OPERATION_SLOTS,
    })
}

pub fn run(
    identities: &BootIdentities,
    fabrication: &FabricationRecord,
    tour: &mut TourProduct,
    display: &mut impl crate::display::PixelTarget,
    session: &mut HidPointerSession,
    controller: &mut XhciReady,
    usb: &UsbDevice,
) -> Result<(), &'static str> {
    arch::early_write(b"CONDUIT_BOOT_STAGE pointer-awaiting-report\n");
    loop {
        let sample = session
            .receive(controller, usb)
            .map_err(|error| error.as_str())?;
        let format = display
            .format()
            .validate()
            .map_err(crate::display::DisplayError::as_str)?;
        let outcome = tour.accept_pointer(
            sample,
            u16::try_from(format.width).map_err(|_| "tour-display-extent-invalid")?,
            u16::try_from(format.height).map_err(|_| "tour-display-extent-invalid")?,
        )?;
        crate::product_front_door::render_tour(tour, display)?;
        emit_sign(&outcome, sample, tour, identities, fabrication);
        arch::early_write(b"CONDUIT_BOOT_STAGE pointer-awaiting-report\n");
    }
}

fn emit_sign(
    outcome: &TourPointerOutcome,
    sample: conduit_semantic_catalog::NormalizedPointerSample,
    tour: &TourProduct,
    identities: &BootIdentities,
    fabrication: &FabricationRecord,
) {
    let (status, subject) = match outcome {
        TourPointerOutcome::Hovered { subject } => ("hovered", subject.as_str()),
        TourPointerOutcome::Selected { subject } => ("selected", subject.as_str()),
    };
    let line = format!(
        "CONDUIT_POINTER_SIGN {{\"schema\":\"conduit.conduitos.pointer-interaction/v1\",\"status\":\"{status}\",\"subject\":\"{subject}\",\"sequence\":{},\"position_x\":{},\"position_y\":{},\"delta_x\":{},\"delta_y\":{},\"primary_pressed\":{},\"queue_capacity\":{},\"revision\":{},\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"proof_class\":\"freestanding-emulator\",\"bounded\":true}}\n",
        sample.sequence,
        sample.position_x,
        sample.position_y,
        sample.delta_x,
        sample.delta_y,
        sample.primary_pressed,
        sample.queue_capacity,
        tour.controller().state().revision,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        identity::hex(&identities.host),
        identity::hex(&identities.boot),
    );
    arch::early_write(line.as_bytes());
}
