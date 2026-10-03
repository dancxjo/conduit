//! Long-lived ordinary product service for the Patchbay lifecycle journey.

mod arrival;
mod face_arrival;
mod face_workspace;
mod guest_join;
mod input_actions;
mod tutorial;
mod workspace_view_sign;
use face_arrival::{FaceArrival, FaceArrivalInput};
use face_workspace::FaceWorkspace;
use input_actions::{ProductControl, action_for, product_control, resident_application_action};
mod journey_sign;
mod owner_action_evidence;
pub(crate) mod transient_sign;
mod workspace_input;
use workspace_input::refresh_with_face as refresh;

use alloc::format;

use conduit_human::KeyTransition;
use conduit_presentation::ApplicationEvent;

use crate::{
    arch::{self, HidKeyboardSession, HidPointerSession, UsbDevice, XhciReady},
    front_door::{FrontDoor, FrontDoorPresenter},
    identity::{self, BootIdentities},
    keyboard_input::{self, ProductInputControl, ProductInputEvent},
    keyboard_text_plan,
    local_rescue::LocalRescueMatcher,
    make::MakeRecord,
    native_compositor::InputRoute,
    offer::CAPABILITY_COUNT,
    offer_make::ImageBoundHostOffer,
    product_bases::{EffectFamily, FRAMEBUFFER_RESOURCE_CLASS, NativeProductBases},
    product_journey::{JourneyStatus, ProductJourney},
    rescue_guest,
};
use journey_sign::emit_journey_sign;

const ENTER: u8 = 40;
const F1: u8 = 58;
const F10: u8 = 67;
const F11: u8 = 68;

#[allow(clippy::too_many_arguments)]
pub fn run(
    identities: &BootIdentities,
    offer: &ImageBoundHostOffer<'_>,
    make: &MakeRecord,
    framebuffer_basis: &conduit_observatory::FramebufferBasis,
    display: &mut impl crate::display::PixelTarget,
    mut hid_session: Option<&mut HidKeyboardSession>,
    controller: &mut XhciReady,
    controller_id: [u8; 32],
    usb: &UsbDevice,
    pointer_session: Option<&mut HidPointerSession>,
    _pointer_usb: Option<&UsbDevice>,
    usb_line_device: Option<&UsbDevice>,
    mut ps2_input: Option<&mut crate::arch::Ps2Input>,
    rescue_matcher: &mut LocalRescueMatcher,
    pending_join: Option<crate::native_boot_join::BootJoinOutcome>,
) -> Result<(), &'static str> {
    let (pending_join, owner_receipt, owner_face, owner_route, owner_return_refusal) =
        match pending_join {
            Some(join) => (
                Some(join.pending),
                join.receipt,
                join.face,
                join.return_route,
                join.return_refusal,
            ),
            None => (None, None, None, None, None),
        };
    let effect_bases = NativeProductBases::observe(offer, framebuffer_basis, usb_line_device)
        .map_err(|_| "product-base-provider-invalid")?;
    effect_bases
        .require(EffectFamily::Framebuffer)
        .map_err(|_| "product-framebuffer-base-unavailable")?;
    effect_bases
        .require_resource(EffectFamily::Framebuffer, FRAMEBUFFER_RESOURCE_CLASS)
        .map_err(|_| "product-framebuffer-resource-unavailable")?;
    if hid_session.is_some() || ps2_input.is_some() {
        effect_bases
            .require(EffectFamily::Keyboard)
            .map_err(|_| "product-keyboard-base-unavailable")?;
    }
    if pointer_session.is_some() {
        effect_bases
            .require(EffectFamily::Pointer)
            .map_err(|_| "product-pointer-base-unavailable")?;
    }
    let host_id = conduit_core::HostId::from(identity::hex(&identities.host));
    let boot_id = conduit_core::BootId::from(identity::hex(&identities.boot));
    let generation = conduit_core::OfferGeneration(offer.generation);
    let mut journey = ProductJourney::new(host_id.clone(), boot_id.clone(), generation)
        .map_err(|error| error.as_str())?;
    let entropy = arch::RdrandEntropy::detect(offer.generation)
        .map_err(|_| "product-surface-authority-entropy-unavailable")?;
    let mut entropy =
        crate::cryptographic_entropy::CryptographicEntropyBase::<_, 1>::admit(entropy)
            .map_err(|_| "product-surface-authority-entropy-invalid")?;
    let mut surface_issuer_key = [0; 32];
    entropy
        .fill(&mut surface_issuer_key)
        .map_err(|_| "product-surface-authority-key-unavailable")?;
    let surface_provider = effect_bases
        .framebuffer_provider(surface_issuer_key)
        .map_err(|_| "product-framebuffer-provider-unavailable")?;
    journey.admit_surface_provider(surface_provider.clone());
    surface_issuer_key.fill(0);
    // The embedded defaults are Crèche inventory, not ProductJourney state.
    let plot = keyboard_text_plan::checked_plot_identity().map_err(|error| error.as_str())?;
    let provisioned_guest = pending_join.is_some();
    let mut front_door = FrontDoor::new(
        host_id.clone(),
        boot_id.clone(),
        generation,
        make.profile_id,
        make.build_id,
        make.image_binding,
        plot.source_document_id,
        plot.checked_plot_id,
        u64::try_from(CAPABILITY_COUNT).unwrap_or(u64::MAX)
            + u64::from(offer.keyboard.is_some())
            + u64::from(offer.pointer.is_some())
            + u64::from(offer.pc_speaker.is_some()),
        !provisioned_guest,
    );
    let owner_face = if let Some(pending) = pending_join {
        guest_join::enter(
            &mut front_door,
            journey.projection(),
            pending,
            owner_receipt,
            owner_face,
            &host_id,
            &boot_id,
            generation,
        )?
    } else {
        arrival::open(&mut front_door, &mut journey, identities, offer, make)?;
        None
    };
    let mut owner_route = owner_face
        .as_ref()
        .filter(|face| face.interactions_admitted())
        .and(owner_route);
    let mut face_arrival = FaceArrival::prepare(
        host_id.clone(),
        boot_id.clone(),
        generation,
        make.build_id,
        framebuffer_basis.base_id.clone(),
        &surface_provider,
    )?;
    let mut face_workspace = FaceWorkspace::prepare(
        host_id.clone(),
        boot_id.clone(),
        generation,
        make.build_id,
        framebuffer_basis.base_id.clone(),
        &surface_provider,
    )?;
    let mut presenter = FrontDoorPresenter::prepare(
        host_id,
        boot_id,
        generation,
        make.profile_id,
        make.image_binding,
        framebuffer_basis.base_id.clone(),
        make.presentation_surface_slots,
    )
    .map_err(|error| error.as_str())?;
    let owner_face_presented = owner_face.is_some();
    let receipt = if let Some(owner_face) = owner_face {
        let admitted = owner_route.as_ref().is_some_and(|route| route.available());
        face_arrival.present_owner_face(owner_face.into_presentation(), admitted, display)?
    } else if provisioned_guest {
        face_arrival.present_pending_join(&front_door, display)?
    } else {
        face_arrival.present_first(&front_door, display)?
    };
    if let Some(reason) = owner_return_refusal {
        face_arrival.show_owner_result(false, false, reason, display)?;
    }
    crate::display::profile::emit_boot_receipt();
    emit_journey_sign(&journey.projection(), make, &receipt);
    arch::early_write(b"CONDUIT_BOOT_STAGE front-door-ready\n");
    if provisioned_guest {
        if front_door.joining_pending() {
            arch::early_write(b"CONDUIT_JOIN_CHECKPOINT awaiting-owner-receipt\n");
        }
    } else {
        arch::early_write(b"CONDUIT_CRECHE_CHECKPOINT ready\n");
    }
    let mut consumed_birth_key = None;
    let mut clock = arch::Clock::new();
    let mut serial = arch::Serial::new();
    let mut interrupts = arch::Interrupts::new();
    let mut idle = arch::Idle::new();
    loop {
        let mut line_requested = false;
        let mut workspace_updates = workspace_input::PendingInput::default();
        let mut interact = |input| {
            if provisioned_guest {
                match input {
                    ProductInputEvent::Service
                        if owner_route.as_ref().is_some_and(|route| !route.available()) =>
                    {
                        owner_route = None;
                        face_arrival.retire_owner_route(display)?;
                        face_arrival.show_owner_result(false, false, "return-expired", display)?;
                        arch::early_write(b"CONDUIT_NATIVE_OWNER_ROUTE {\"schema\":\"conduit.conduitos/native-owner-route@1\",\"status\":\"expired\"}\n");
                    }
                    ProductInputEvent::LocalRescue(local) => {
                        rescue_guest::observe(identities, rescue_matcher, local, true);
                    }
                    ProductInputEvent::Key(event) if owner_face_presented => {
                        let input = match face_arrival.accept_guest_key(event, display) {
                            Ok(input) => input,
                            Err(reason) => {
                                arch::early_write(b"CONDUIT_NATIVE_OWNER_INPUT {\"schema\":\"conduit.conduitos/native-owner-input@1\",\"status\":\"refused\",\"code\":\"");
                                arch::early_write(reason.as_bytes());
                                arch::early_write(b"\"}\n");
                                face_arrival.show_local_refusal(reason, display)?;
                                return Ok(ProductInputControl::Continue);
                            }
                        };
                        if let Some((show, interaction)) = input {
                            let outcome = owner_route
                                .as_mut()
                                .ok_or("native-owner-return-unavailable")
                                .and_then(|route| route.submit(*identities, &show, &interaction));
                            match outcome {
                                Ok(outcome) => {
                                    let admitted =
                                        owner_route.as_ref().is_some_and(|route| route.available());
                                    owner_action_evidence::emit(&show, &interaction, &outcome);
                                    let refreshed = outcome.face.is_some();
                                    if let Some(face) = outcome.face {
                                        face_arrival.present_owner_face(
                                            face.into_presentation(),
                                            admitted,
                                            display,
                                        )?;
                                    } else {
                                        face_arrival.retire_owner_route(display)?;
                                    }
                                    face_arrival.show_owner_result(
                                        outcome.accepted,
                                        refreshed,
                                        &outcome.code,
                                        display,
                                    )?;
                                }
                                Err(reason) => {
                                    arch::early_write(b"CONDUIT_NATIVE_OWNER_ACTION {\"schema\":\"conduit.conduitos/native-owner-action@1\",\"status\":\"");
                                    arch::early_write(if reason == "control-outcome-unknown" {
                                        b"unknown"
                                    } else {
                                        b"refused"
                                    });
                                    arch::early_write(b"\",\"code\":\"");
                                    arch::early_write(reason.as_bytes());
                                    arch::early_write(b"\"}\n");
                                    owner_route = None;
                                    face_arrival.retire_owner_route(display)?;
                                    face_arrival
                                        .show_owner_result(false, false, reason, display)?;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                return Ok(ProductInputControl::Continue);
            }
            let event = match input {
                ProductInputEvent::Service => {
                    if let Some(receipt) = workspace_updates.service_with_face(
                        &mut front_door,
                        &journey,
                        &mut presenter,
                        &mut face_workspace,
                        display,
                        true,
                    )? {
                        emit_journey_sign(&journey.projection(), make, &receipt);
                    }
                    return Ok(ProductInputControl::Continue);
                }
                ProductInputEvent::LocalRescue(local) => {
                    rescue_guest::observe(identities, rescue_matcher, local, true);
                    return Ok(ProductInputControl::Continue);
                }
                ProductInputEvent::Key(event) => event,
                ProductInputEvent::Lost(_) => {
                    if matches!(
                        journey.status(),
                        JourneyStatus::Planned | JourneyStatus::QuiescentAwaitingInput
                    ) {
                        journey
                            .input_lost(crate::product_journey::JourneyLossKind::InputDevice)
                            .map_err(|error| error.as_str())?;
                        let receipt = refresh(
                            &mut front_door,
                            &journey,
                            &mut presenter,
                            &mut face_workspace,
                            display,
                        )?;
                        emit_journey_sign(&journey.projection(), make, &receipt);
                    }
                    return Ok(ProductInputControl::Continue);
                }
            };
            if consumed_birth_key == Some(event.usage()) {
                if event.transition() == KeyTransition::Released {
                    consumed_birth_key = None;
                }
                return Ok(ProductInputControl::Continue);
            }
            // A held key keeps its original Plot owner across surface changes,
            // including when the compositor currently has no keyboard target.
            if journey.owns_key_release(event) {
                workspace_updates.accept(event, &mut journey, &mut front_door)?;
                return Ok(ProductInputControl::Continue);
            }
            let keyboard_target = if front_door.creche_open() {
                face_arrival.route_keyboard()?
            } else if face_workspace.active() {
                face_workspace.route_keyboard()?
            } else {
                matches!(
                    presenter.route_keyboard().map_err(|error| error.as_str())?,
                    InputRoute::Delivered(_)
                )
            };
            if !keyboard_target {
                return Ok(ProductInputControl::Continue);
            }
            if product_control(event.usage()) == Some(ProductControl::UsbLine)
                && usb_line_device.is_some()
            {
                if event.transition() == KeyTransition::Released {
                    effect_bases
                        .require(EffectFamily::Line)
                        .map_err(|_| "product-line-base-unavailable")?;
                    line_requested = true;
                    return Ok(ProductInputControl::Yield);
                }
                return Ok(ProductInputControl::Continue);
            }
            if event.transition() == KeyTransition::Pressed
                && product_control(event.usage()) == Some(ProductControl::Tour)
            {
                face_workspace.relinquish()?;
                tutorial::select(&mut journey, &mut front_door, &mut presenter, display, make)?;
                return Ok(ProductInputControl::Continue);
            }
            if face_workspace.active() {
                if event.transition() == KeyTransition::Pressed && event.usage() == 41 {
                    let receipt = face_workspace.leave(&front_door, &mut presenter, display)?;
                    emit_journey_sign(&journey.projection(), make, &receipt);
                    arch::early_write(b"CONDUIT_WORKSPACE_FACE returned-to-application\n");
                    return Ok(ProductInputControl::Continue);
                }
                // Existing application shortcuts remain available after an
                // explicit return to its surface. F3 belongs to the Face
                // diagram only while this Mask is active; Tour F3 is unchanged.
                if event.transition() == KeyTransition::Pressed
                    && matches!(event.usage(), F1 | F10 | F11)
                {
                    face_workspace.leave(&front_door, &mut presenter, display)?;
                } else if event.transition() == KeyTransition::Pressed
                    && matches!(event.usage(), 61..=65 | 77)
                {
                    if let Some(action) = action_for(event.usage(), &front_door, &journey) {
                        let receipt = face_workspace.invoke_product_action(
                            action,
                            &mut front_door,
                            &mut journey,
                            display,
                            identities,
                            offer,
                            make,
                        )?;
                        emit_journey_sign(&journey.projection(), make, &receipt);
                    }
                    return Ok(ProductInputControl::Continue);
                } else {
                    if let Some(receipt) = face_workspace.accept_key(
                        event,
                        &mut front_door,
                        &mut journey,
                        display,
                        identities,
                        offer,
                        make,
                    )? {
                        emit_journey_sign(&journey.projection(), make, &receipt);
                        workspace_view_sign::emit(&front_door, &journey, &receipt)?;
                    }
                    return Ok(ProductInputControl::Continue);
                }
            }
            if event.transition() == KeyTransition::Pressed
                && event.usage() == 59
                && !front_door.exact_details_open()
                && journey.projection().body_id.is_some()
            {
                let receipt = face_workspace.enter(&front_door, &mut presenter, display)?;
                emit_journey_sign(&journey.projection(), make, &receipt);
                arch::early_write(b"CONDUIT_WORKSPACE_FACE shown\n");
                return Ok(ProductInputControl::Continue);
            }
            if front_door.home_open()
                && product_control(event.usage()) != Some(ProductControl::Lifecycle)
            {
                match front_door
                    .accept_home(event, front_door.revision())
                    .map_err(|error| error.as_str())?
                {
                    crate::front_door::HomeInput::Unchanged => {}
                    crate::front_door::HomeInput::Changed => {
                        presenter
                            .present(&front_door, display)
                            .map_err(|error| error.as_str())?;
                        arch::early_write(
                            format!(
                                "CONDUIT_HOME_STATE {} {}\n",
                                front_door
                                    .home_view()
                                    .map_or("closed", crate::front_door::HomeView::as_str),
                                front_door.home_selection().unwrap_or(0)
                            )
                            .as_bytes(),
                        );
                    }
                    crate::front_door::HomeInput::OpenTour => {
                        tutorial::select(
                            &mut journey,
                            &mut front_door,
                            &mut presenter,
                            display,
                            make,
                        )?;
                    }
                    crate::front_door::HomeInput::OpenPatchbay => {
                        for _ in 0..crate::native_workset::NATIVE_PLOT_CAPACITY {
                            if journey
                                .workspace_projection()
                                .and_then(|workspace| {
                                    workspace.plots.into_iter().find(|plot| plot.foreground)
                                })
                                .is_some_and(|plot| plot.title == "Patchbay")
                            {
                                break;
                            }
                            journey
                                .select_next_plot(journey.revision())
                                .map_err(|error| error.as_str())?;
                        }
                        front_door.close_home().map_err(|error| error.as_str())?;
                        let receipt = refresh(
                            &mut front_door,
                            &journey,
                            &mut presenter,
                            &mut face_workspace,
                            display,
                        )?;
                        emit_journey_sign(&journey.projection(), make, &receipt);
                        arch::early_write(b"CONDUIT_HOME_CHECKPOINT patchbay-opened\n");
                    }
                    crate::front_door::HomeInput::OpenCreche => {
                        arch::early_write(
                            b"CONDUIT_HOME_CHECKPOINT creche-unavailable-after-birth\n",
                        );
                        presenter
                            .present(&front_door, display)
                            .map_err(|error| error.as_str())?;
                    }
                    crate::front_door::HomeInput::OpenPlot(index) => {
                        let inventory = crate::native_workset::inventory();
                        let requested =
                            inventory.get(index).ok_or("home-plot-selection-invalid")?;
                        for _ in 0..crate::native_workset::NATIVE_PLOT_CAPACITY {
                            if journey
                                .workspace_projection()
                                .and_then(|workspace| {
                                    workspace.plots.into_iter().find(|plot| plot.foreground)
                                })
                                .is_some_and(|plot| plot.title == requested.title())
                            {
                                break;
                            }
                            journey
                                .select_next_plot(journey.revision())
                                .map_err(|error| error.as_str())?;
                        }
                        front_door.close_home().map_err(|error| error.as_str())?;
                        let receipt = refresh(
                            &mut front_door,
                            &journey,
                            &mut presenter,
                            &mut face_workspace,
                            display,
                        )?;
                        emit_journey_sign(&journey.projection(), make, &receipt);
                        arch::early_write(
                            format!("CONDUIT_HOME_CHECKPOINT plot-opened {}\n", requested.name())
                                .as_bytes(),
                        );
                    }
                    crate::front_door::HomeInput::RunPlot(index) => {
                        let inventory = crate::native_workset::inventory();
                        let requested = inventory.get(index).ok_or("home-run-selection-invalid")?;
                        for _ in 0..crate::native_workset::NATIVE_PLOT_CAPACITY {
                            if journey
                                .workspace_projection()
                                .and_then(|workspace| {
                                    workspace.plots.into_iter().find(|plot| plot.foreground)
                                })
                                .is_some_and(|plot| plot.title == requested.title())
                            {
                                break;
                            }
                            journey
                                .select_next_plot(journey.revision())
                                .map_err(|error| error.as_str())?;
                        }
                        front_door.close_home().map_err(|error| error.as_str())?;
                        let receipt = refresh(
                            &mut front_door,
                            &journey,
                            &mut presenter,
                            &mut face_workspace,
                            display,
                        )?;
                        emit_journey_sign(&journey.projection(), make, &receipt);
                        arch::early_write(
                            format!("CONDUIT_HOME_CHECKPOINT plot-run {}\n", requested.name())
                                .as_bytes(),
                        );
                    }
                }
                return Ok(ProductInputControl::Continue);
            }
            if front_door.creche_open() {
                match face_arrival.accept_key(
                    event,
                    &mut front_door,
                    &mut journey,
                    &mut presenter,
                    display,
                    make,
                )? {
                    FaceArrivalInput::Continue => {}
                    FaceArrivalInput::Born => {
                        let (observation, play) = face_arrival.next_publication_sequences();
                        face_workspace.continue_after_birth(observation, play);
                        consumed_birth_key = Some(event.usage());
                    }
                }
                return Ok(ProductInputControl::Continue);
            }
            if event.transition() == KeyTransition::Pressed
                && product_control(event.usage()) == Some(ProductControl::Escape)
                && journey.projection().body_id.is_some()
                && !front_door.exact_details_open()
            {
                front_door.open_home().map_err(|error| error.as_str())?;
                let receipt = refresh(
                    &mut front_door,
                    &journey,
                    &mut presenter,
                    &mut face_workspace,
                    display,
                )?;
                emit_journey_sign(&journey.projection(), make, &receipt);
                arch::early_write(b"CONDUIT_HOME_CHECKPOINT returned\n");
                return Ok(ProductInputControl::Continue);
            }
            if !front_door.exact_details_open() {
                if tutorial::navigate(event, &journey, &mut front_door, &mut presenter, display)? {
                    return Ok(ProductInputControl::Continue);
                }

                if event.transition() == KeyTransition::Pressed
                    && matches!(event.usage(), 40 | F1 | F10 | F11)
                    && let Some(view) = front_door.application_view().cloned()
                {
                    let Some(action) = tutorial::selected_action(event.usage(), &front_door, &view)
                    else {
                        return Ok(ProductInputControl::Continue);
                    };
                    if journey.foreground_is_tutorial() {
                        tutorial::activate(
                            &view,
                            &action,
                            &mut journey,
                            &mut front_door,
                            &mut presenter,
                            display,
                            identities,
                            offer,
                            make,
                        )?;
                        return Ok(ProductInputControl::Continue);
                    }
                    journey
                        .accept_application_event(&tutorial::event(&view, &action))
                        .map_err(|error| error.as_str())?;
                    match journey.take_application_request() {
                        Some(crate::native_workset::NativeApplicationRequest::Tutorial(_)) => {
                            return Err("tutorial-request-outside-current-tutorial");
                        }
                        Some(crate::native_workset::NativeApplicationRequest::RunTour {
                            chapter: 0,
                            stage: 2,
                        }) => {
                            let mut prepared = crate::tour_play::prepare_morse_stage(
                                identities,
                                offer,
                                make.build_id,
                            )
                            .map_err(|error| error.as_str())?;
                            let evidence = crate::tour_play::run_morse_stage(
                                &mut prepared,
                                &mut clock,
                                &mut serial,
                                &mut interrupts,
                                &mut idle,
                            )
                            .map_err(|_| "resident-tour-morse-play-refused")?;
                            journey
                                .complete_tour_run(&evidence)
                                .map_err(|error| error.as_str())?;
                            arch::early_write(
                                b"CONDUIT_WORKSPACE_CHECKPOINT resident-tour-fanout-ran\n",
                            );
                        }
                        Some(crate::native_workset::NativeApplicationRequest::RunTour {
                            chapter,
                            stage,
                        }) => {
                            let (mut prepared, specimen_id, expected) =
                                crate::tour_play::prepare_stage(
                                    identities,
                                    offer,
                                    make.build_id,
                                    chapter,
                                    stage,
                                )
                                .map_err(|error| error.as_str())?;
                            let evidence = crate::tour_play::run_stage(
                                &mut prepared,
                                specimen_id,
                                expected,
                                &mut clock,
                                &mut serial,
                                &mut interrupts,
                                &mut idle,
                            )
                            .map_err(|_| "resident-tour-play-refused")?;
                            journey
                                .complete_tour_run(&evidence)
                                .map_err(|error| error.as_str())?;
                            arch::early_write(b"CONDUIT_WORKSPACE_CHECKPOINT resident-tour-ran\n");
                        }
                        Some(crate::native_workset::NativeApplicationRequest::EditCurrent(
                            request @ patchbay_application::PatchbayApplicationRequest::ChangeMasks { .. },
                        )) => {
                            journey
                                .replan_masks(
                                    request,
                                    identities,
                                    offer,
                                    make.build_id,
                                )
                                .map_err(|error| error.as_str())?;
                            arch::early_write(
                                b"CONDUIT_WORKSPACE_CHECKPOINT patchbay-presenters-replanned\n",
                            );
                        }
                        Some(crate::native_workset::NativeApplicationRequest::EditCurrent(_)) => {
                            arch::early_write(
                                b"CONDUIT_WORKSPACE_CHECKPOINT patchbay-edit-requested\n",
                            );
                        }
                        Some(crate::native_workset::NativeApplicationRequest::OpenPatchbay) => {
                            return Err("resident-patchbay-request-not-routed");
                        }
                        None => {}
                    }
                    let receipt = refresh(
                        &mut front_door,
                        &journey,
                        &mut presenter,
                        &mut face_workspace,
                        display,
                    )?;
                    emit_journey_sign(&journey.projection(), make, &receipt);
                    return Ok(ProductInputControl::Continue);
                }
                if let Some(changed) = workspace_input::select(event, &mut journey)? {
                    if changed {
                        let receipt = refresh(
                            &mut front_door,
                            &journey,
                            &mut presenter,
                            &mut face_workspace,
                            display,
                        )?;
                        emit_journey_sign(&journey.projection(), make, &receipt);
                    }
                    return Ok(ProductInputControl::Continue);
                }
                if journey.status() == JourneyStatus::QuiescentAwaitingInput
                    && journey.foreground_input_owner().is_some()
                    && product_control(event.usage()).is_none()
                {
                    workspace_updates.accept(event, &mut journey, &mut front_door)?;
                    return Ok(ProductInputControl::Continue);
                }
                if product_control(event.usage()) == Some(ProductControl::SelectNextPlot) {
                    return Ok(ProductInputControl::Continue);
                }
            }
            if event.transition() == KeyTransition::Pressed
                && let Some(action) = action_for(event.usage(), &front_door, &journey)
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
                        make.build_id,
                        front_door.revision(),
                    )
                    .map_err(|error| error.as_str())?;
                arch::early_write(
                    format!("CONDUIT_PRODUCT_ACTION applied {}\n", action.as_str()).as_bytes(),
                );
                let receipt = refresh(
                    &mut front_door,
                    &journey,
                    &mut presenter,
                    &mut face_workspace,
                    display,
                )?;
                emit_journey_sign(&journey.projection(), make, &receipt);
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
            return Err("product-input-ended-without-control-request");
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
        let peer_host_id = conduit_core::HostId::from(crate::product_usb_line::HARNESS_HOST_ID);
        let peer_boot_id = conduit_core::BootId::from(crate::product_usb_line::HARNESS_BOOT_ID);
        let peer_proof =
            conduit_body::MembershipProofId::bind("conduitos/product/reviewed-qemu-line-peer")
                .map_err(|_| "product-usb-line-membership-proof-invalid")?;
        let mut peer_part = None;
        line.run(
            controller,
            line_device,
            body_id.as_str(),
            |status, line_id, value| {
                match status {
                    crate::front_door::ConnectivityStatus::PeerAttached => {
                        peer_part = Some(
                            journey
                                .admit_line_peer(
                                    peer_host_id.clone(),
                                    peer_boot_id.clone(),
                                    peer_proof.clone(),
                                )
                                .map_err(|error| error.as_str())?,
                        );
                    }
                    crate::front_door::ConnectivityStatus::Lost => {
                        journey
                            .observe_line_peer_offline(
                                peer_part
                                    .as_ref()
                                    .ok_or("product-usb-line-peer-not-admitted")?,
                                &peer_boot_id,
                            )
                            .map_err(|error| error.as_str())?;
                    }
                    _ => {}
                }
                front_door
                    .observe_connectivity(crate::front_door::ConnectivityProjection {
                        line_id: line_id.into(),
                        status,
                        value: value.map(Into::into),
                        body_id: body_id.clone(),
                    })
                    .map_err(|error| error.as_str())?;
                let receipt = refresh(
                    &mut front_door,
                    &journey,
                    &mut presenter,
                    &mut face_workspace,
                    display,
                )?;
                emit_journey_sign(&journey.projection(), make, &receipt);
                Ok(())
            },
        )?;
        // The keyboard receive was published before the F12 release yielded
        // ownership to the Line. Its exact pending transfer remains the
        // hand-back boundary when the finite Line session ends.
        arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-resumed-after-line\n");
    }
}

#[cfg(test)]
mod input_routing_tests;
