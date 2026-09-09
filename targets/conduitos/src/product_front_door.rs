//! Long-lived ordinary product service for the Patchbay lifecycle journey.

mod arrival;
mod input_actions;
use input_actions::{action_for, form_receives_input};
mod journey_sign;
mod scroll_input;
mod tour_sign;
pub(crate) mod transient_sign;

use alloc::format;

use conduit_human::KeyTransition;
use conduit_presentation::{ApplicationEvent, ApplicationEventKind};
use conduit_tour_model::{OPEN_PATCHBAY_ACTION_ID, RUN_ACTION_ID, TourTransientKind};

use crate::{
    arch::{self, HidKeyboardSession, HidPointerSession, UsbDevice, XhciReady},
    fabrication::FabricationRecord,
    front_door::{FrontDoor, FrontDoorPresenter},
    identity::{self, BootIdentities},
    keyboard_input::{self, ProductInputControl, ProductInputEvent},
    local_rescue::LocalRescueMatcher,
    native_compositor::InputRoute,
    offer::CAPABILITY_COUNT,
    offer_fabrication::ImageBoundHostOffer,
    product_journey::{JourneyStatus, ProductJourney},
    rescue_guest,
    tour_product::TourProduct,
    tour_shell::TourShellPresenter,
};
use journey_sign::emit_journey_sign;
use tour_sign::emit_tour_sign;
use transient_sign::{emit_dismissed_transient, emit_shown_transient};

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
    mut hid_session: Option<&mut HidKeyboardSession>,
    controller: &mut XhciReady,
    controller_id: [u8; 32],
    usb: &UsbDevice,
    mut pointer_session: Option<&mut HidPointerSession>,
    pointer_usb: Option<&UsbDevice>,
    usb_line_device: Option<&UsbDevice>,
    mut ps2_input: Option<&mut crate::arch::Ps2Input>,
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
    arrival::open(
        &mut front_door,
        &mut journey,
        identities,
        offer,
        fabrication,
    )?;
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
    let receipt = presenter
        .present(&front_door, display)
        .map_err(|error| error.as_str())?;
    emit_journey_sign(&journey.projection(), fabrication, &receipt);
    arch::early_write(b"CONDUIT_BOOT_STAGE front-door-ready\nCONDUIT_CRECHE_CHECKPOINT ready\n");
    let mut tour = TourProduct::canonical(1);
    let mut shell = TourShellPresenter::prepare(
        conduit_core::HostId::from(identity::hex(&identities.host)),
        conduit_core::BootId::from(identity::hex(&identities.boot)),
        generation,
        fabrication.profile_id,
        fabrication.image_binding,
        framebuffer_basis.base_id.clone(),
        fabrication.presentation_surface_slots,
    )
    .map_err(|error| error.as_str())?;
    let mut tour_open = false;
    let mut consumed_birth_key = None;
    let mut clock = arch::Clock::new();
    let mut serial = arch::Serial::new();
    let mut interrupts = arch::Interrupts::new();
    let mut idle = arch::Idle::new();
    loop {
        let mut line_requested = false;
        let mut interact = |input| {
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
            if consumed_birth_key == Some(event.usage()) {
                if event.transition() == KeyTransition::Released {
                    consumed_birth_key = None;
                }
                return Ok(ProductInputControl::Continue);
            }
            let keyboard_route = if tour_open {
                shell.route_keyboard().map_err(|error| error.as_str())?
            } else {
                presenter.route_keyboard().map_err(|error| error.as_str())?
            };
            if matches!(keyboard_route, InputRoute::NoTarget) {
                return Ok(ProductInputControl::Continue);
            }
            if event.usage() == F12 && usb_line_device.is_some() {
                if event.transition() == KeyTransition::Released {
                    line_requested = true;
                    return Ok(ProductInputControl::Yield);
                }
                return Ok(ProductInputControl::Continue);
            }
            if event.transition() == KeyTransition::Pressed && event.usage() == F9 && !tour_open {
                presenter.suspend().map_err(|error| error.as_str())?;
                tour_open = true;
                let shell_receipt = shell
                    .present_with_lifecycle(&tour, &journey.projection(), display)
                    .map_err(|error| error.as_str())?;
                emit_tour_sign(&tour, None, &shell_receipt, identities, fabrication);
                arch::early_write(b"CONDUIT_TOUR_CHECKPOINT workspace-opened\n");
                return Ok(ProductInputControl::Continue);
            }
            if tour_open && event.transition() == KeyTransition::Pressed {
                if scroll_input::accept(event.usage(), &mut shell, display)? {
                    return Ok(ProductInputControl::Continue);
                }
                if event.usage() == ESCAPE {
                    if shell.has_transient() {
                        let dismissal = shell
                            .dismiss_transient(display)
                            .map_err(|error| error.as_str())?;
                        emit_dismissed_transient(&dismissal, false, identities, fabrication);
                        arch::early_write(b"CONDUIT_TOUR_CHECKPOINT transient-dismissed\n");
                        return Ok(ProductInputControl::Continue);
                    }
                    if tour
                        .controller()
                        .state()
                        .selected_patchbay_subject
                        .is_some()
                    {
                        tour.dismiss_inspector().map_err(|error| error.as_str())?;
                        shell
                            .present_with_lifecycle(&tour, &journey.projection(), display)
                            .map_err(|error| error.as_str())?;
                        arch::early_write(b"CONDUIT_TOUR_CHECKPOINT gear-inspector-dismissed\n");
                        return Ok(ProductInputControl::Continue);
                    }
                    tour_open = false;
                    shell.suspend().map_err(|error| error.as_str())?;
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
                    let update = match tour.accept(
                        &event,
                        identities,
                        offer,
                        fabrication.build_id,
                        &mut clock,
                        &mut serial,
                        &mut interrupts,
                        &mut idle,
                    ) {
                        Ok(update) => update,
                        Err(error) if error.controller_refusal().is_some() => {
                            let refusal = error.controller_refusal().expect("matched refusal");
                            let receipt = shell
                                .show_transient(
                                    &tour,
                                    TourTransientKind::Refusal,
                                    refusal.as_str(),
                                    display,
                                )
                                .map_err(|error| error.as_str())?;
                            emit_shown_transient(
                                &receipt,
                                Some(refusal.as_str()),
                                &shell,
                                identities,
                                fabrication,
                            )?;
                            arch::early_write(b"CONDUIT_TOUR_CHECKPOINT refusal-transient-shown\n");
                            return Ok(ProductInputControl::Continue);
                        }
                        Err(error) => return Err(error.as_str()),
                    };
                    let shell_receipt = shell
                        .present_with_lifecycle(&tour, &journey.projection(), display)
                        .map_err(|error| error.as_str())?;
                    emit_tour_sign(
                        &tour,
                        Some(&update),
                        &shell_receipt,
                        identities,
                        fabrication,
                    );
                    if update.play.is_some() {
                        arch::early_write(b"\n");
                        let receipt = shell
                            .show_transient(
                                &tour,
                                TourTransientKind::Confirmation,
                                "Play completed",
                                display,
                            )
                            .map_err(|error| error.as_str())?;
                        emit_shown_transient(&receipt, None, &shell, identities, fabrication)?;
                        arch::early_write(
                            b"CONDUIT_TOUR_CHECKPOINT confirmation-transient-shown\n",
                        );
                    } else if action == OPEN_PATCHBAY_ACTION_ID {
                        let receipt = shell
                            .show_transient(
                                &tour,
                                TourTransientKind::Chooser,
                                "Choose a Patchbay Gear",
                                display,
                            )
                            .map_err(|error| error.as_str())?;
                        emit_shown_transient(&receipt, None, &shell, identities, fabrication)?;
                        arch::early_write(b"CONDUIT_TOUR_CHECKPOINT chooser-transient-shown\n");
                    }
                    return Ok(
                        if action == OPEN_PATCHBAY_ACTION_ID && pointer_session.is_some() {
                            ProductInputControl::Yield
                        } else {
                            ProductInputControl::Continue
                        },
                    );
                }
            }
            if !tour_open && front_door.creche_open() {
                match front_door
                    .accept_creche(event, front_door.revision())
                    .map_err(|e| e.as_str())?
                {
                    crate::front_door::ArrivalInput::Unchanged => {}
                    crate::front_door::ArrivalInput::Changed => {
                        presenter
                            .present(&front_door, display)
                            .map_err(|e| e.as_str())?;
                        arch::early_write(b"CONDUIT_CRECHE_CHECKPOINT edited\n");
                    }
                    crate::front_door::ArrivalInput::Birth(selection) => {
                        consumed_birth_key = Some(event.usage());
                        arrival::birth_and_wake(
                            selection,
                            &mut front_door,
                            &mut journey,
                            &mut presenter,
                            display,
                            identities,
                            offer,
                            fabrication,
                        )?;
                    }
                }
                return Ok(ProductInputControl::Continue);
            }
            if journey.status() == JourneyStatus::Playing
                && form_receives_input(tour_open, front_door.exact_details_open(), event.usage())
            {
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
        };
        if let Some(ps2) = ps2_input.as_deref_mut() {
            keyboard_input::run_ps2_product(ps2, &mut interact)?;
        } else {
            let session = hid_session
                .as_deref_mut()
                .ok_or("front-door-keyboard-realization-missing")?;
            keyboard_input::run_product(session, controller, usb, &mut interact)?;
        }
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
    if let Some(ps2) = ps2_input {
        return crate::product_pointer::run_ps2(
            identities,
            fabrication,
            &mut tour,
            &mut shell,
            display,
            ps2,
        );
    }
    let (pointer_session, pointer_usb) = pointer_session
        .take()
        .zip(pointer_usb)
        .ok_or("front-door-pointer-realization-missing")?;
    crate::product_pointer::run(
        identities,
        fabrication,
        &mut tour,
        &mut shell,
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

#[cfg(test)]
mod input_routing_tests;
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
