//! Long-lived ordinary product service for the Patchbay lifecycle journey.

use alloc::{format, string::String};

use conduit_human::KeyTransition;
use conduit_presentation::{
    ApplicationEvent, ApplicationEventKind, GraphicsCommand, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, LayoutRect,
};
use conduit_tour_model::{OPEN_PATCHBAY_ACTION_ID, RUN_ACTION_ID};

use crate::{
    arch::{self, HidKeyTransition, HidKeyboardSession, HidPointerSession, UsbDevice, XhciReady},
    fabrication::FabricationRecord,
    front_door::{FrontDoor, FrontDoorPresenter},
    identity::{self, BootIdentities},
    keyboard_input::{self, ProductInputControl, ProductInputEvent},
    local_rescue::LocalRescueMatcher,
    offer::CAPABILITY_COUNT,
    offer_fabrication::ImageBoundHostOffer,
    product_bindings::binding_for_usage,
    product_journey::{JourneyAction, JourneyProjection, JourneyStatus, ProductJourney},
    rescue_guest,
    tour_product::{TourProduct, TourProductUpdate},
};

const ENTER: u8 = 40;
const ESCAPE: u8 = 41;
const F9: u8 = 66;
const F10: u8 = 67;
const F11: u8 = 68;
const F12: u8 = 69;

#[allow(clippy::too_many_arguments)]
pub fn run(
    identities: &BootIdentities,
    offer: &ImageBoundHostOffer<'_>,
    fabrication: &FabricationRecord,
    framebuffer_basis: &conduit_observatory::FramebufferBasis,
    display: &mut impl crate::display::PixelTarget,
    hid_session: &mut HidKeyboardSession,
    controller: &mut XhciReady,
    controller_id: [u8; 32],
    usb: &UsbDevice,
    pointer_session: Option<&mut HidPointerSession>,
    pointer_usb: Option<&UsbDevice>,
    usb_line_device: Option<&UsbDevice>,
    rescue_matcher: &mut LocalRescueMatcher,
) -> Result<(), &'static str> {
    let host_id = conduit_core::HostId::from(identity::hex(&identities.host));
    let boot_id = conduit_core::BootId::from(identity::hex(&identities.boot));
    let generation = conduit_core::OfferGeneration(offer.generation);
    let mut journey = ProductJourney::new(host_id.clone(), boot_id.clone(), generation)
        .map_err(|error| error.as_str())?;
    let form = journey.form().clone();
    let mut front_door = FrontDoor::new(
        host_id.clone(),
        boot_id.clone(),
        generation,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        form.source_document_id,
        form.checked_form_id,
        u64::try_from(CAPABILITY_COUNT).unwrap_or(u64::MAX)
            + u64::from(offer.keyboard.is_some())
            + u64::from(offer.pointer.is_some())
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
    let mut tour = TourProduct::canonical(1);
    let mut tour_open = false;
    let mut clock = arch::Clock::new();
    let mut serial = arch::Serial::new();
    let mut interrupts = arch::Interrupts::new();
    let mut idle = arch::Idle::new();
    loop {
        let mut line_requested = false;
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
                    return Ok(ProductInputControl::Continue);
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
            if event.usage() == F12 && usb_line_device.is_some() {
                if event.transition() == KeyTransition::Released {
                    line_requested = true;
                    return Ok(ProductInputControl::Yield);
                }
                return Ok(ProductInputControl::Continue);
            }
            if event.transition() == KeyTransition::Pressed && event.usage() == F9 && !tour_open {
                tour_open = true;
                render_tour(&tour, display)?;
                emit_tour_sign(&tour, None, identities, fabrication);
                arch::early_write(b"CONDUIT_TOUR_CHECKPOINT workspace-opened\n");
                return Ok(ProductInputControl::Continue);
            }
            if tour_open && event.transition() == KeyTransition::Pressed {
                if event.usage() == ESCAPE {
                    tour_open = false;
                    presenter
                        .present(&front_door, display)
                        .map_err(|error| error.as_str())?;
                    arch::early_write(b"CONDUIT_TOUR_CHECKPOINT world-returned\n");
                    return Ok(ProductInputControl::Continue);
                }
                if let Some(action) = tour_action(event.usage()) {
                    let event = ApplicationEvent {
                        revision: tour.controller().state().revision,
                        action: action.into(),
                        kind: ApplicationEventKind::Activate,
                        value: alloc::vec::Vec::new(),
                    };
                    let update = tour
                        .accept(
                            &event,
                            identities,
                            offer,
                            fabrication.build_id,
                            &mut clock,
                            &mut serial,
                            &mut interrupts,
                            &mut idle,
                        )
                        .map_err(|error| error.as_str())?;
                    render_tour(&tour, display)?;
                    if update.play.is_some() {
                        arch::early_write(b"\n");
                    }
                    emit_tour_sign(&tour, Some(&update), identities, fabrication);
                    return Ok(
                        if action == OPEN_PATCHBAY_ACTION_ID && pointer_session.is_some() {
                            ProductInputControl::Yield
                        } else {
                            ProductInputControl::Continue
                        },
                    );
                }
            }
            if journey.status() == JourneyStatus::Playing && !is_control_transition(transition) {
                if journey
                    .accept_play_input(event)
                    .map_err(|error| error.as_str())?
                {
                    let receipt = refresh(&mut front_door, &journey, &mut presenter, display)?;
                    emit_journey_sign(&journey.projection(), fabrication, &receipt);
                }
                return Ok(ProductInputControl::Continue);
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
                return Ok(ProductInputControl::Continue);
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
            Ok(ProductInputControl::Continue)
        })?;
        if !line_requested {
            break;
        }
        let body_id = journey
            .projection()
            .body_id
            .ok_or("product-usb-line-body-absent")?;
        let line_device = usb_line_device.ok_or("product-usb-line-realization-absent")?;
        let ready = crate::arch::prepare_ftdi_line(
            controller,
            line_device,
            crate::boot::executable_physical_address,
        )
        .map_err(|error| error.as_str())?;
        let mut line =
            crate::product_usb_line::prepare(identities, controller_id, line_device, ready)?;
        line.run(
            controller,
            line_device,
            body_id.as_str(),
            |status, line_id, value| {
                front_door
                    .observe_connectivity(crate::front_door::ConnectivityProjection {
                        line_id: line_id.into(),
                        status,
                        value: value.map(Into::into),
                        body_id: body_id.clone(),
                    })
                    .map_err(|error| error.as_str())?;
                presenter
                    .present(&front_door, display)
                    .map_err(|error| error.as_str())?;
                Ok(())
            },
        )?;
    }
    let (pointer_session, pointer_usb) = pointer_session
        .zip(pointer_usb)
        .ok_or("front-door-pointer-realization-missing")?;
    crate::product_pointer::run(
        identities,
        fabrication,
        &mut tour,
        display,
        pointer_session,
        controller,
        pointer_usb,
    )
}

fn tour_action(usage: u8) -> Option<&'static str> {
    match usage {
        F10 => Some(RUN_ACTION_ID),
        F11 => Some(OPEN_PATCHBAY_ACTION_ID),
        _ => None,
    }
}

pub(crate) fn render_tour(
    tour: &TourProduct,
    display: &mut impl crate::display::PixelTarget,
) -> Result<(), &'static str> {
    let format = display
        .format()
        .validate()
        .map_err(crate::display::DisplayError::as_str)?;
    let scene = tour
        .scene(
            u16::try_from(format.width).map_err(|_| "tour-display-extent-invalid")?,
            u16::try_from(format.height).map_err(|_| "tour-display-extent-invalid")?,
        )
        .map_err(|error| error.as_str())?;
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: u16::try_from(format.width).map_err(|_| "tour-display-extent-invalid")?,
        height: u16::try_from(format.height).map_err(|_| "tour-display-extent-invalid")?,
    };
    let mut background = GraphicsScene::empty();
    background
        .push(
            GraphicsCommand::rect(
                bounds,
                bounds,
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            )
            .map_err(|_| "tour-background-scene-refused")?,
        )
        .map_err(|_| "tour-background-scene-refused")?;
    crate::display::render_scene(display, &background)
        .map_err(crate::display::DisplayError::as_str)?;
    crate::display::render_scene(display, &scene)
        .map(|_| ())
        .map_err(crate::display::DisplayError::as_str)
}

fn emit_tour_sign(
    tour: &TourProduct,
    update: Option<&TourProductUpdate>,
    identities: &BootIdentities,
    fabrication: &FabricationRecord,
) {
    let state = tour.controller().state();
    let play = update.and_then(|value| value.play.as_ref());
    let line = format!(
        "CONDUIT_TOUR_SIGN {{\"schema\":\"conduit.conduitos.tour/v1\",\"status\":\"{}\",\"revision\":{},\"specimen_id\":\"{}\",\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"source_document_id\":{},\"checked_form_id\":{},\"expanded_form_id\":{},\"plan_id\":{},\"active_play_id\":{},\"result\":{},\"proof_class\":\"freestanding-emulator\",\"bounded\":true}}\n",
        match state.phase {
            conduit_tour_model::TourWorkspacePhase::LessonReady => "tour-opened",
            conduit_tour_model::TourWorkspacePhase::ResultVisible => "result-visible",
            conduit_tour_model::TourWorkspacePhase::PatchbayOpen => "patchbay-open",
        },
        state.revision,
        state.specimen_id,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        identity::hex(&identities.host),
        identity::hex(&identities.boot),
        json_optional(play.map(|value| value.source_document_id.as_str())),
        json_optional(play.map(|value| value.checked_form_id.as_str())),
        json_optional(play.map(|value| value.expanded_form_id.as_str())),
        json_optional(play.map(|value| value.plan_id.as_str())),
        json_optional(play.map(|value| value.active_play_id.as_str())),
        json_optional(state.result.as_deref()),
    );
    arch::early_write(line.as_bytes());
}

fn json_optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), |value| format!("\"{value}\""))
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
        "CONDUIT_PRODUCT_JOURNEY {{\"status\":\"{}\",\"revision\":{},\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"offer_generation\":{},\"source_document_id\":\"{}\",\"checked_form_id\":\"{}\",\"expanded_form_id\":\"{}\",\"body_id\":{},\"born_sign_id\":{},\"part_id\":{},\"wake_id\":{},\"plan_id\":{},\"active_play_id\":{},\"gear_ids\":{},\"port_ids\":{},\"cord_ids\":{},\"presentation_id\":\"{}\",\"manifestation_id\":\"{}\",\"presenter_implementation_id\":\"{}\",\"input_sign_id\":{},\"result_sign_id\":{},\"result\":{},\"request_id\":{}}}\n",
        projection.status.as_str(),
        projection.revision,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        projection.host_id.as_str(),
        projection.boot_id.as_str(),
        projection.offer_generation.0,
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
