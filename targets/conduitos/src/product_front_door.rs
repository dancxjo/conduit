//! Long-lived ordinary product service for the Patchbay lifecycle journey.

use alloc::{format, string::String};

use conduit_core::KeyTransition;

use crate::{
    arch::{self, HidKeyTransition, HidKeyboardSession, UsbDevice, XhciReady},
    fabrication::FabricationRecord,
    front_door::{FrontDoor, FrontDoorPresenter},
    identity::{self, BootIdentities},
    keyboard_input::{self, ProductInputEvent},
    local_rescue::LocalRescueMatcher,
    offer::CAPABILITY_COUNT,
    offer_fabrication::ImageBoundHostOffer,
    product_bindings::binding_for_usage,
    product_journey::{JourneyAction, JourneyProjection, JourneyStatus, ProductJourney},
    rescue_guest,
};

const ENTER: u8 = 40;

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
    rescue_matcher: &mut LocalRescueMatcher,
) -> Result<(), &'static str> {
    let host_id = conduit_core::HostId::from(identity::hex(&identities.host));
    let boot_id = conduit_core::BootId::from(identity::hex(&identities.boot));
    let generation = conduit_core::OfferGeneration(offer.generation);
    let mut journey = ProductJourney::new(host_id.clone(), boot_id.clone(), generation)
        .map_err(|error| error.as_str())?;
    let seed = journey.seed().clone();
    let mut front_door = FrontDoor::new(
        host_id.clone(),
        boot_id.clone(),
        generation,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        seed.source_document_id,
        seed.checked_form_id,
        u64::try_from(CAPABILITY_COUNT).unwrap_or(u64::MAX)
            + u64::from(offer.keyboard.is_some())
            + u64::from(offer.pc_speaker.is_some()),
        true,
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
    keyboard_input::run_product(hid_session, controller, usb, |input| {
        let transition = match input {
            ProductInputEvent::Transition(transition) => transition,
            ProductInputEvent::Lost(_) => {
                if matches!(
                    journey.status(),
                    JourneyStatus::Planned | JourneyStatus::Playing
                ) {
                    journey.input_lost().map_err(|error| error.as_str())?;
                    let receipt = refresh(&mut front_door, &journey, &mut presenter, display)?;
                    emit_journey_sign(&journey.projection(), fabrication, &receipt);
                }
                return Ok(());
            }
        };
        rescue_guest::observe(
            identities,
            rescue_matcher,
            transition.into_local_rescue(),
            true,
        );
        let event = crate::keyboard_bridge::portable_key_event(
            transition.usage(),
            transition.pressed(),
            transition.modifiers(),
        )
        .map_err(|_| "front-door-key-event-invalid")?;
        if journey.status() == JourneyStatus::Playing && !is_control_transition(transition) {
            if journey
                .accept_play_input(event)
                .map_err(|error| error.as_str())?
            {
                let receipt = refresh(&mut front_door, &journey, &mut presenter, display)?;
                emit_journey_sign(&journey.projection(), fabrication, &receipt);
            }
            return Ok(());
        }
        if event.transition() == KeyTransition::Pressed
            && let Some(action) = action_for(transition.usage(), &front_door, &journey)
        {
            let semantic_action = front_door
                .resolve_action(action, front_door.revision())
                .map_err(|error| error.as_str())?;
            let request = journey
                .next_request(action, semantic_action.target, front_door.revision())
                .map_err(|error| error.as_str())?;
            journey
                .apply(
                    request,
                    identities,
                    offer,
                    fabrication.build_id,
                    front_door.revision(),
                )
                .map_err(|error| error.as_str())?;
            let receipt = refresh(&mut front_door, &journey, &mut presenter, display)?;
            emit_journey_sign(&journey.projection(), fabrication, &receipt);
            return Ok(());
        }
        let revision = front_door.revision();
        if front_door
            .accept(event, revision)
            .map_err(|error| error.as_str())?
        {
            presenter
                .present(&front_door, display)
                .map_err(|error| error.as_str())?;
            if front_door.exact_details_open() {
                let (label, value) = front_door.current_detail();
                arch::early_write(
                    format!(
                        "CONDUIT_FRONT_DOOR_SIGN {{\"status\":\"details-opened\",\"label\":\"{label}\",\"value\":\"{value}\"}}\n"
                    )
                    .as_bytes(),
                );
            }
        }
        Ok(())
    })
}

fn action_for(
    usage: u8,
    front_door: &FrontDoor,
    journey: &ProductJourney,
) -> Option<JourneyAction> {
    if usage == ENTER && !front_door.exact_details_open() {
        return Some(JourneyAction::OpenBack);
    }
    let action = binding_for_usage(usage)?.action;
    if action == JourneyAction::Stop
        && !matches!(
            journey.status(),
            JourneyStatus::Playing | JourneyStatus::ResultVisible
        )
    {
        return None;
    }
    Some(action)
}

fn is_control_transition(transition: HidKeyTransition) -> bool {
    binding_for_usage(transition.usage()).is_some()
}

fn refresh(
    front_door: &mut FrontDoor,
    journey: &ProductJourney,
    presenter: &mut FrontDoorPresenter,
    display: &mut impl crate::display::PixelTarget,
) -> Result<crate::native_compositor::CompositionReceipt, &'static str> {
    front_door
        .observe_journey(journey.projection())
        .map_err(|error| error.as_str())?;
    presenter
        .present(front_door, display)
        .map_err(|error| error.as_str())
}

fn emit_journey_sign(
    projection: &JourneyProjection,
    fabrication: &FabricationRecord,
    receipt: &crate::native_compositor::CompositionReceipt,
) {
    let line = format!(
        "CONDUIT_PRODUCT_JOURNEY {{\"status\":\"{}\",\"revision\":{},\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"offer_generation\":{},\"seed_id\":\"{}\",\"source_document_id\":\"{}\",\"checked_form_id\":\"{}\",\"expanded_form_id\":\"{}\",\"body_id\":{},\"born_sign_id\":{},\"part_id\":{},\"wake_id\":{},\"plan_id\":{},\"active_play_id\":{},\"gear_ids\":{},\"port_ids\":{},\"cord_ids\":{},\"presentation_id\":\"{}\",\"manifestation_id\":\"{}\",\"presenter_implementation_id\":\"{}\",\"input_sign_id\":{},\"result_sign_id\":{},\"result\":{},\"request_id\":{}}}\n",
        projection.status.as_str(),
        projection.revision,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        projection.host_id.as_str(),
        projection.boot_id.as_str(),
        projection.offer_generation.0,
        projection.seed_id.as_str(),
        projection.source_document_id.as_str(),
        projection.checked_form_id.as_str(),
        projection.expanded_form_id.as_str(),
        json_identity(
            projection
                .body_id
                .as_ref()
                .map(conduit_body::BodyId::as_str)
        ),
        json_identity(
            projection
                .born_sign_id
                .as_ref()
                .map(conduit_core::SignId::as_str)
        ),
        json_identity(
            projection
                .part_id
                .as_ref()
                .map(conduit_body::PartId::as_str)
        ),
        json_identity(
            projection
                .wake_id
                .as_ref()
                .map(conduit_body::WakeId::as_str)
        ),
        json_identity(
            projection
                .plan_id
                .as_ref()
                .map(conduit_core::PlanId::as_str)
        ),
        json_identity(
            projection
                .active_play_id
                .as_ref()
                .map(conduit_core::ActivePlayId::as_str)
        ),
        json_array(&projection.gear_ids),
        json_array(&projection.port_ids),
        json_array(&projection.cord_ids),
        receipt.presentation_id.as_str(),
        receipt.manifestation_id.as_str(),
        receipt.presenter_implementation_id.as_str(),
        json_identity(
            projection
                .input_sign_id
                .as_ref()
                .map(conduit_core::SignId::as_str)
        ),
        json_identity(
            projection
                .result_sign_id
                .as_ref()
                .map(conduit_core::SignId::as_str)
        ),
        json_identity(projection.result.as_deref()),
        json_identity(projection.last_request_id.as_deref()),
    );
    arch::early_write(line.as_bytes());
}

fn json_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{value}\""))
            .collect::<alloc::vec::Vec<_>>()
            .join(",")
    )
}

fn json_identity(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), |value| format!("\"{value}\""))
}
