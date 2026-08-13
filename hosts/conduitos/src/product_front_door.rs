//! Long-lived ordinary product service loop for the zero-Body Patchbay entrance.

use alloc::format;

use crate::{
    arch::{self, HidKeyboardSession, UsbDevice, XhciReady},
    fabrication::FabricationRecord,
    front_door::{FrontDoor, FrontDoorPresenter, Status},
    identity::{self, BootIdentities},
    keyboard_input,
    keyboard_plan::PreparedKeyboardPlay,
    local_rescue::LocalRescueMatcher,
    offer::CAPABILITY_COUNT,
    offer_fabrication::ImageBoundHostOffer,
    ordinary_plan, rescue_guest,
};

#[allow(clippy::too_many_arguments)]
pub fn run(
    identities: &BootIdentities,
    offer: &ImageBoundHostOffer<'_>,
    fabrication: &FabricationRecord,
    framebuffer_basis: &conduit_observatory::FramebufferBasis,
    display: &mut impl crate::display::PixelTarget,
    hid_session: &mut HidKeyboardSession,
    controller: &mut XhciReady,
    usb: &UsbDevice,
    keyboard: &PreparedKeyboardPlay,
    rescue_matcher: &mut LocalRescueMatcher,
) -> Result<(), &'static str> {
    let standard = ordinary_plan::prepare(identities, offer, fabrication.build_id)
        .map_err(|error| error.as_str())?;
    let host_id = conduit_core::HostId::from(identity::hex(&identities.host));
    let boot_id = conduit_core::BootId::from(identity::hex(&identities.boot));
    let generation = conduit_core::OfferGeneration(offer.generation);
    let mut front_door = FrontDoor::new(
        host_id.clone(),
        boot_id.clone(),
        generation,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        standard.plan.source_document_id.clone(),
        standard.plan.checked_form_id.clone(),
        u64::try_from(CAPABILITY_COUNT).unwrap_or(u64::MAX)
            + u64::from(offer.keyboard.is_some())
            + u64::from(offer.pc_speaker.is_some()),
    );
    let mut presenter = FrontDoorPresenter::prepare(
        host_id,
        boot_id,
        generation,
        fabrication.profile_id,
        fabrication.image_binding,
        framebuffer_basis.base_id.clone(),
        fabrication.presentation_surface_slots,
    )
    .map_err(|error| error.as_str())?;
    presenter
        .present(&front_door, display)
        .map_err(|error| error.as_str())?;
    arch::early_write(b"CONDUIT_BOOT_STAGE front-door-ready\n");
    keyboard_input::run_interactive(
        hid_session,
        controller,
        usb,
        keyboard,
        |transition| {
            rescue_guest::observe(
                identities,
                rescue_matcher,
                transition.into_local_rescue(),
                true,
            )
        },
        |transition| {
            let event = crate::keyboard_bridge::portable_key_event(
                transition.usage(),
                transition.pressed(),
                transition.modifiers(),
            )
            .map_err(|_| "front-door-key-event-invalid")?;
            let revision = front_door.revision();
            if !front_door
                .accept(event, revision)
                .map_err(|error| error.as_str())?
            {
                return Ok(());
            }
            presenter
                .present(&front_door, display)
                .map_err(|error| error.as_str())?;
            match front_door.status() {
                Status::SeedOpened => {
                    let sign = format!(
                        "CONDUIT_FRONT_DOOR_SIGN {{\"status\":\"seed-opened\",\"seed_id\":\"{}\",\"body\":null,\"wake\":null,\"plan\":null,\"play\":null,\"effects\":0}}\n",
                        front_door.seed_id().as_str(),
                    );
                    arch::early_write(sign.as_bytes());
                }
                Status::DetailsOpened => arch::early_write(
                    b"CONDUIT_FRONT_DOOR_SIGN {\"status\":\"details-opened\",\"body\":null}\n",
                ),
                Status::World => {}
            }
            Ok(())
        },
    )
}
