//! Long-lived ordinary product service for the Patchbay lifecycle journey.

mod arrival;
mod controller_io;
mod face_arrival;
mod face_workspace;
mod guest_join;
mod home_input;
mod input_actions;
mod platform;
mod pointer_input;
mod prebirth;
mod runtime;
mod startup;
pub use platform::run;
mod tutorial;
mod workspace_view_sign;
use face_arrival::{FaceArrival, FaceArrivalInput};
use face_workspace::FaceWorkspace;
use input_actions::{ProductControl, action_for, product_control, resident_application_action};
mod journey_sign;
mod native_action_evidence;
mod owner_action_evidence;
mod owner_selection;
mod patchbay_edit;
pub(crate) mod transient_sign;
mod workspace_input;
use workspace_input::refresh_with_face as refresh;

use alloc::format;

use conduit_face::ApplicationEvent;
use conduit_human::KeyTransition;

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use crate::display::PixelTarget as _;

use crate::{
    arch,
    front_door::{FrontDoor, FrontDoorPresenter},
    identity::{self, BootIdentities},
    keyboard_text_plan,
    make::MakeRecord,
    native_compositor::InputRoute,
    native_observation::{Phase, Progress},
    offer::CAPABILITY_COUNT,
    offer_make::ImageBoundHostOffer,
    product_bases::{EffectFamily, FRAMEBUFFER_RESOURCE_CLASS, NativeProductBases},
    product_journey::{JourneyStatus, ProductJourney},
};
use controller_io::{ControllerControl, ControllerInput};
use journey_sign::emit_journey_sign;

const ENTER: u8 = 40;
const F1: u8 = 58;
const F5: u8 = 62;
const F10: u8 = 67;
const F11: u8 = 68;

#[allow(clippy::too_many_arguments)]
fn run_controller(
    identities: &BootIdentities,
    offer: &ImageBoundHostOffer<'_>,
    make: &MakeRecord,
    framebuffer_basis: &conduit_observatory::FramebufferBasis,
    display: &mut platform::DisplaySink<'_, impl crate::display::PixelTarget>,
    platform: &mut platform::RootPlatform<'_>,
    runtime: &mut runtime::RootRuntime,
    effect_bases: NativeProductBases,
    surface_provider: crate::product_bases::NativeSurfaceProvider,
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
    effect_bases
        .require(EffectFamily::Framebuffer)
        .map_err(|_| "product-framebuffer-base-unavailable")?;
    effect_bases
        .require_resource(EffectFamily::Framebuffer, FRAMEBUFFER_RESOURCE_CLASS)
        .map_err(|_| "product-framebuffer-resource-unavailable")?;
    if platform.keyboard_available() {
        effect_bases
            .require(EffectFamily::Keyboard)
            .map_err(|_| "product-keyboard-base-unavailable")?;
    }
    if platform.pointer_available() {
        effect_bases
            .require(EffectFamily::Pointer)
            .map_err(|_| "product-pointer-base-unavailable")?;
    }
    let host_id = conduit_core::HostId::from(identity::hex(&identities.host));
    let boot_id = conduit_core::BootId::from(identity::hex(&identities.boot));
    let generation = conduit_core::OfferGeneration(offer.generation);
    let mut journey = Progress::result(Phase::IdentityResolve, "product-journey", || {
        ProductJourney::new(host_id.clone(), boot_id.clone(), generation)
    })
    .map_err(|error| error.as_str())?;
    if effect_bases
        .require(EffectFamily::Framebuffer)
        .map_err(|_| "product-framebuffer-base-unavailable")?
        != &surface_provider.entry
    {
        return Err("product-framebuffer-provider-changed");
    }
    // Prepare one isolated renderer for this display. Presenter/provider clones
    // share only this explicit owner; no global service or implicit restart.
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    let (surface_provider, display_owner) = {
        let format = display
            .format()
            .validate()
            .map_err(|error| error.as_str())?;
        if surface_provider.entry.base_id != framebuffer_basis.base_id
            || format.width != framebuffer_basis.width
            || format.height != framebuffer_basis.height
            || format.pitch != framebuffer_basis.pitch_bytes
            || format.bits_per_pixel != framebuffer_basis.bits_per_pixel
        {
            return Err("compositor-display-grant-mismatch");
        }
        let grant = crate::protected_compositor::display_grant::DisplayGrant::new(
            host_id.clone(),
            boot_id.clone(),
            generation,
            framebuffer_basis.base_id.clone(),
            format,
        )
        .map_err(|error| error.as_str())?;
        let owner = crate::protected_compositor::display_owner::DisplayOwner::prepare(grant)
            .map_err(|_| "compositor-display-preparation-refused")?;
        let report = owner.borrow().preparation_report();
        arch::early_write(format!(
            "CONDUIT_COMPOSITOR_SERVICE_PREPARED {{\"schema\":\"conduit.conduitos/compositor-service-preparation@1\",\"service_reserved_bytes\":{},\"staging_logical_bytes\":{},\"staging_reserved_bytes\":{},\"initialization_entries\":{},\"initialization_privilege_transitions\":{}}}\n",
            report.service_reserved_bytes,
            report.staging_logical_bytes,
            report.staging_reserved_bytes,
            report.service_cost_at_preparation.entries,
            report.service_cost_at_preparation.privilege_transitions,
        ).as_bytes());
        let mut provider = surface_provider;
        provider.display_owner = Some(owner.clone());
        (provider, owner)
    };
    journey.admit_surface_provider(surface_provider.clone());
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
        arrival::open(&mut front_door, &mut journey, identities, offer)?;
        None
    };
    let mut owner_route = owner_face
        .as_ref()
        .filter(|face| face.interactions_admitted())
        .and(owner_route);
    let mut current_owner_face = owner_face.clone();
    let mut face_arrival = Progress::result(Phase::MaskPreparation, "arrival", || {
        FaceArrival::prepare(
            host_id.clone(),
            boot_id.clone(),
            generation,
            make.build_id,
            framebuffer_basis.base_id.clone(),
            &surface_provider,
            owner_face.as_ref().and_then(|face| face.route()),
        )
    })?;
    let mut face_workspace = Progress::result(Phase::MaskPreparation, "workspace", || {
        FaceWorkspace::prepare(
            host_id.clone(),
            boot_id.clone(),
            generation,
            make.build_id,
            framebuffer_basis.base_id.clone(),
            &surface_provider,
        )
    })?;
    let mut presenter = FrontDoorPresenter::prepare(
        host_id,
        boot_id,
        generation,
        make.profile_id,
        make.image_binding,
        framebuffer_basis.base_id.clone(),
        make.face_surface_slots,
    )
    .map_err(|error| error.as_str())?;
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    presenter
        .attach_display_owner(display_owner)
        .map_err(|error| error.as_str())?;
    let owner_face_presented = owner_face.is_some();
    let mut owner_route_standby = owner_route.as_ref().is_some_and(|route| route.available());
    let receipt = Progress::result(Phase::FirstShow, "arrival", || {
        if let Some(owner_face) = owner_face {
            face_arrival.present_owner_face(owner_face.into_face(), false, display)
        } else if provisioned_guest {
            face_arrival.present_pending_join(&front_door, display)
        } else {
            prebirth::enter_commons(&mut front_door, &mut face_arrival, &mut presenter, display)
        }
    })?;
    crate::native_observation::point(crate::native_observation::Stage::FirstFramebufferCommit);
    if owner_route_standby {
        face_arrival.show_owner_standby(display)?;
        arch::early_write(b"CONDUIT_NATIVE_OWNER_ROUTE {\"schema\":\"conduit.conduitos/native-owner-route@1\",\"status\":\"standby\",\"activation\":\"F5\"}\n");
    }
    if let Some(reason) = owner_return_refusal {
        face_arrival.show_owner_result(false, false, reason, display)?;
    }
    let mut pointer_state = pointer_input::PointerState::new();
    let pointer_admitted = platform.pointer_available();
    let line_available = platform.line_available();
    if pointer_admitted {
        pointer_input::render(
            &pointer_state,
            provisioned_guest,
            &mut face_arrival,
            &front_door,
            &mut presenter,
            &mut face_workspace,
            display,
        )?;
    }
    crate::display::profile::emit_boot_receipt();
    emit_journey_sign(&journey.projection(), make, &receipt);
    arch::early_write(b"CONDUIT_BOOT_STAGE front-door-ready\n");
    if provisioned_guest {
        if front_door.joining_pending() {
            arch::early_write(b"CONDUIT_JOIN_CHECKPOINT awaiting-owner-receipt\n");
        }
    } else {
        arch::early_write(b"CONDUIT_COMMONS_CHECKPOINT ready\n");
    }
    let mut consumed_birth_key = None;
    let mut first_input_wait = Some(Progress::begin(Phase::InputWait, "first-decoded-input"));
    loop {
        let mut line_requested = false;
        let mut workspace_updates = workspace_input::PendingInput::default();
        let mut interact = |input| {
            if matches!(input, ControllerInput::Key(_) | ControllerInput::Pointer(_))
                && let Some(span) = first_input_wait.take()
            {
                span.finish();
            }
            if matches!(input, ControllerInput::Service) && pointer_admitted {
                pointer_input::render(
                    &pointer_state,
                    provisioned_guest,
                    &mut face_arrival,
                    &front_door,
                    &mut presenter,
                    &mut face_workspace,
                    display,
                )?;
            }
            if let ControllerInput::PointerLost(reason) = input {
                pointer_state.retire();
                arch::early_write(format!("CONDUIT_POINTER_LOST {reason}\n").as_bytes());
                return Ok(ControllerControl::Continue);
            }
            if let ControllerInput::Pointer(sample) = input {
                pointer_input::dispatch(
                    &mut pointer_state,
                    sample,
                    provisioned_guest,
                    &mut face_arrival,
                    &mut front_door,
                    &mut journey,
                    &mut presenter,
                    &mut face_workspace,
                    display,
                    make,
                    identities,
                    offer,
                )?;
                return Ok(ControllerControl::Continue);
            }
            if provisioned_guest {
                match input {
                    ControllerInput::Service
                        if owner_route.as_ref().is_some_and(|route| !route.available()) =>
                    {
                        owner_route = None;
                        owner_route_standby = false;
                        face_arrival.retire_owner_route(display)?;
                        face_arrival.show_owner_result(false, false, "return-expired", display)?;
                        arch::early_write(b"CONDUIT_NATIVE_OWNER_ROUTE {\"schema\":\"conduit.conduitos/native-owner-route@1\",\"status\":\"expired\"}\n");
                    }
                    ControllerInput::Key(event) if owner_face_presented => {
                        if let Some(disposition) = owner_selection::dispatch(
                            event,
                            *identities,
                            &mut face_arrival,
                            &mut current_owner_face,
                            &mut owner_route,
                            owner_route_standby,
                            display,
                        )? {
                            if let owner_selection::SelectionDisposition::Refused(reason) =
                                disposition
                            {
                                owner_selection::record_refusal(reason);
                            }
                            return Ok(ControllerControl::Continue);
                        }
                        if owner_route_standby
                            && event.transition() == KeyTransition::Pressed
                            && event.usage() == F5
                        {
                            let prior = current_owner_face
                                .as_ref()
                                .ok_or("native-owner-face-absent")?;
                            let fresh = match owner_route
                                .as_mut()
                                .ok_or("native-owner-return-unavailable")?
                                .refresh(*identities, prior)
                            {
                                Ok(face) => face,
                                Err(reason) => {
                                    arch::early_write(b"CONDUIT_NATIVE_OWNER_REFRESH {\"schema\":\"conduit.conduitos/native-owner-refresh@1\",\"status\":\"refused\",\"code\":\"");
                                    arch::early_write(reason.as_bytes());
                                    arch::early_write(b"\"}\n");
                                    owner_route = None;
                                    owner_route_standby = false;
                                    face_arrival.retire_owner_route(display)?;
                                    face_arrival
                                        .show_owner_result(false, false, reason, display)?;
                                    return Ok(ControllerControl::Continue);
                                }
                            };
                            face_arrival
                                .activate_refreshed_owner_route(fresh.face().clone(), display)?;
                            current_owner_face = Some(fresh);
                            let route = owner_route
                                .as_mut()
                                .ok_or("native-owner-return-unavailable")?;
                            match face_arrival.acknowledge_owner_show(route, *identities) {
                                Ok(()) => {
                                    owner_route_standby = false;
                                    arch::early_write(b"CONDUIT_NATIVE_OWNER_ROUTE {\"schema\":\"conduit.conduitos/native-owner-route@1\",\"status\":\"active\"}\n");
                                }
                                Err(reason) => {
                                    arch::early_write(b"CONDUIT_NATIVE_OWNER_SHOW_ACK {\"status\":\"refused\",\"code\":\"");
                                    arch::early_write(reason.as_bytes());
                                    arch::early_write(b"\"}\n");
                                    owner_route = None;
                                    owner_route_standby = false;
                                    face_arrival.retire_owner_route(display)?;
                                    face_arrival
                                        .show_owner_result(false, false, reason, display)?;
                                }
                            }
                            return Ok(ControllerControl::Continue);
                        }
                        let input = match face_arrival.accept_guest_key(event, display) {
                            Ok(input) => input,
                            Err(reason) => {
                                arch::early_write(b"CONDUIT_NATIVE_OWNER_INPUT {\"schema\":\"conduit.conduitos/native-owner-input@1\",\"status\":\"refused\",\"code\":\"");
                                arch::early_write(reason.as_bytes());
                                arch::early_write(b"\"}\n");
                                face_arrival.show_local_refusal(reason, display)?;
                                return Ok(ControllerControl::Continue);
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
                                            face.into_face(),
                                            admitted,
                                            display,
                                        )?;
                                        if admitted
                                            && let Some(route) = owner_route.as_mut()
                                            && let Err(reason) = face_arrival
                                                .acknowledge_owner_show(route, *identities)
                                        {
                                            owner_route = None;
                                            face_arrival.retire_owner_route(display)?;
                                            face_arrival
                                                .show_owner_result(false, false, reason, display)?;
                                            return Ok(ControllerControl::Continue);
                                        }
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
                return Ok(ControllerControl::Continue);
            }
            let event = match input {
                ControllerInput::Service => {
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
                    return Ok(ControllerControl::Continue);
                }
                ControllerInput::Pointer(_) | ControllerInput::PointerLost(_) => {
                    unreachable!("physical pointer handled before keyboard dispatch")
                }
                ControllerInput::Key(event) => event,
                ControllerInput::Lost(_) => {
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
                    return Ok(ControllerControl::Continue);
                }
            };
            if consumed_birth_key == Some(event.usage()) {
                if event.transition() == KeyTransition::Released {
                    consumed_birth_key = None;
                }
                return Ok(ControllerControl::Continue);
            }
            // A held key keeps its original Plot owner across surface changes,
            // including when the compositor currently has no keyboard target.
            if journey.owns_key_release(event) {
                workspace_updates.accept(event, &mut journey, &mut front_door)?;
                return Ok(ControllerControl::Continue);
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
                return Ok(ControllerControl::Continue);
            }
            if product_control(event.usage()) == Some(ProductControl::UsbLine) && line_available {
                if event.transition() == KeyTransition::Released {
                    effect_bases
                        .require(EffectFamily::Line)
                        .map_err(|_| "product-line-base-unavailable")?;
                    line_requested = true;
                    return Ok(ControllerControl::Yield);
                }
                return Ok(ControllerControl::Continue);
            }
            if event.transition() == KeyTransition::Pressed
                && product_control(event.usage()) == Some(ProductControl::Tour)
                && journey.projection().body_id.is_some()
            {
                face_workspace.relinquish()?;
                tutorial::select(&mut journey, &mut front_door, &mut presenter, display, make)?;
                return Ok(ControllerControl::Continue);
            }
            if face_workspace.active() {
                if face_workspace.editor_surface() && event.usage() == 43 {
                    if event.transition() == KeyTransition::Pressed {
                        face_workspace.leave(&front_door, &mut presenter, display)?;
                        journey
                            .select_next_plot(journey.revision())
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
                    return Ok(ControllerControl::Continue);
                }
                if !face_workspace.editor_surface()
                    && event.transition() == KeyTransition::Pressed
                    && event.usage() == 41
                {
                    let receipt = face_workspace.leave(&front_door, &mut presenter, display)?;
                    emit_journey_sign(&journey.projection(), make, &receipt);
                    arch::early_write(b"CONDUIT_WORKSPACE_FACE returned-to-application\n");
                    return Ok(ControllerControl::Continue);
                }
                // Existing application shortcuts remain available after an
                // explicit return to its surface. F3 belongs to the Face
                // diagram only while this Mask is active; Tour F3 is unchanged.
                if event.transition() == KeyTransition::Pressed
                    && (matches!(event.usage(), F1 | F10 | F11)
                        || (face_workspace.editor_surface() && event.usage() == 41))
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
                    return Ok(ControllerControl::Continue);
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
                    return Ok(ControllerControl::Continue);
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
                return Ok(ControllerControl::Continue);
            }
            if front_door.home_open()
                && (journey.projection().body_id.is_none()
                    || product_control(event.usage()) != Some(ProductControl::Lifecycle))
            {
                home_input::accept(
                    event,
                    &mut front_door,
                    &mut journey,
                    &mut presenter,
                    &mut face_workspace,
                    &mut face_arrival,
                    display,
                    make,
                )?;
                return Ok(ControllerControl::Continue);
            }
            if prebirth::back(
                event,
                &mut front_door,
                &journey,
                &mut face_arrival,
                &mut presenter,
                display,
                make,
            )? {
                return Ok(ControllerControl::Continue);
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
                        consumed_birth_key = Some(event.usage());
                        startup::after_birth(
                            &face_arrival,
                            &mut front_door,
                            &mut journey,
                            &mut presenter,
                            &mut face_workspace,
                            display,
                            identities,
                            offer,
                            make,
                        )?;
                    }
                }
                return Ok(ControllerControl::Continue);
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
                return Ok(ControllerControl::Continue);
            }
            if !front_door.exact_details_open() {
                if tutorial::navigate(event, &journey, &mut front_door, &mut presenter, display)? {
                    return Ok(ControllerControl::Continue);
                }

                if event.transition() == KeyTransition::Pressed
                    && matches!(event.usage(), 40 | F1 | F10 | F11)
                    && let Some(view) = front_door.application_view().cloned()
                {
                    let Some(action) = tutorial::selected_action(event.usage(), &front_door, &view)
                    else {
                        return Ok(ControllerControl::Continue);
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
                        return Ok(ControllerControl::Continue);
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
                            let evidence = runtime.run_tour(
                                identities, offer, make.build_id,
                                controller_io::RunTourStage { chapter: 0, stage: 2 },
                            )?;
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
                            let evidence = runtime.run_tour(
                                identities, offer, make.build_id,
                                controller_io::RunTourStage { chapter, stage },
                            )?;
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
                        Some(crate::native_workset::NativeApplicationRequest::EditCurrent(request)) => {
                            patchbay_edit::open_source_editor(&mut front_door, &mut journey, request, identities)?;
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
                    return Ok(ControllerControl::Continue);
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
                    return Ok(ControllerControl::Continue);
                }
                if journey.status() == JourneyStatus::QuiescentAwaitingInput
                    && journey.foreground_input_owner().is_some()
                    && product_control(event.usage()).is_none()
                {
                    workspace_updates.accept(event, &mut journey, &mut front_door)?;
                    return Ok(ControllerControl::Continue);
                }
                if product_control(event.usage()) == Some(ProductControl::SelectNextPlot) {
                    return Ok(ControllerControl::Continue);
                }
            }
            if event.transition() == KeyTransition::Pressed
                && let Some(action) = action_for(event.usage(), &front_door, &journey)
            {
                let receipt = face_workspace.invoke_from_presenter(
                    action,
                    &mut front_door,
                    &mut journey,
                    &mut presenter,
                    display,
                    identities,
                    offer,
                    make,
                )?;
                arch::early_write(
                    format!("CONDUIT_PRODUCT_ACTION handled {}\n", action.as_str()).as_bytes(),
                );
                emit_journey_sign(&journey.projection(), make, &receipt);
                return Ok(ControllerControl::Continue);
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
            Ok(ControllerControl::Continue)
        };
        platform.run_input(&mut interact)?;
        if !line_requested {
            return Err("product-input-ended-without-control-request");
        }
        let body_id = journey
            .projection()
            .body_id
            .ok_or("product-usb-line-body-absent")?;
        let peer_host_id = conduit_core::HostId::from(crate::product_usb_line::HARNESS_HOST_ID);
        let peer_boot_id = conduit_core::BootId::from(crate::product_usb_line::HARNESS_BOOT_ID);
        let peer_proof =
            conduit_body::MembershipProofId::bind("conduitos/product/reviewed-qemu-line-peer")
                .map_err(|_| "product-usb-line-membership-proof-invalid")?;
        let mut peer_part = None;
        platform.run_line(body_id.as_str(), |observation| {
            let platform::ConnectivityObservation {
                status,
                line_id,
                value,
            } = observation;
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
        })?;
        // The keyboard receive was published before the F12 release yielded
        // ownership to the Line. Its exact pending transfer remains the
        // hand-back boundary when the finite Line session ends.
        arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-resumed-after-line\n");
    }
}

#[cfg(test)]
mod input_routing_tests;
