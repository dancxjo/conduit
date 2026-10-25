//! Long-lived bounded input orchestration after the initial acknowledged Show.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    identities: &BootIdentities,
    offer: &ImageBoundHostOffer<'_>,
    make: &MakeRecord,
    display: &mut platform::DisplaySink<'_, impl crate::display::PixelTarget>,
    platform: &mut platform::RootPlatform<'_>,
    runtime: &mut runtime::RootRuntime,
    effect_bases: NativeProductBases,
    mut journey: ProductJourney,
    mut front_door: FrontDoor,
    mut presenter: FrontDoorPresenter,
    mut face_arrival: FaceArrival,
    mut face_workspace: FaceWorkspace,
    mut pointer_state: pointer_input::PointerState,
    provisioned_guest: bool,
    pointer_admitted: bool,
    line_available: bool,
    owner_face_presented: bool,
    mut owner_route_standby: bool,
    mut owner_route: Option<crate::native_owner_return::NativeOwnerReturnRoute>,
    mut current_owner_face: Option<crate::native_guest_face::NativeGuestFace>,
) -> Result<(), &'static str> {
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
                return guest_input::dispatch(
                    input,
                    owner_face_presented,
                    identities,
                    &mut face_arrival,
                    &mut current_owner_face,
                    &mut owner_route,
                    &mut owner_route_standby,
                    display,
                );
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
                    application_request::accept(
                        &view,
                        &action,
                        &mut journey,
                        &mut front_door,
                        runtime,
                        identities,
                        offer,
                        make,
                    )?;
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
        line_session::run(
            &mut journey,
            &mut front_door,
            &mut presenter,
            &mut face_workspace,
            display,
            platform,
            make,
        )?;
        // The keyboard receive was published before the F12 release yielded
        // ownership to the Line. Its exact pending transfer remains the
        // hand-back boundary when the finite Line session ends.
        arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-resumed-after-line\n");
    }
}
