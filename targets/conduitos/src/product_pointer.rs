//! Prepared USB pointer realization and ordinary Tour interaction service.

use alloc::format;

use conduit_tour_model::TourPointerOutcome;

use crate::{
    arch::{self, HidPointerReady, HidPointerSession, UsbDevice, XhciReady},
    fabrication::FabricationRecord,
    identity::{self, BootIdentities},
    native_compositor::{NativeCompositorError, RoutedPointer},
    pointer_offer::PointerRealization,
    tour_product::TourProduct,
    tour_shell::{
        ShellPresentationReceipt, TRANSIENT_SURFACE, TourShellError, TourShellPresenter,
        WORKSPACE_SURFACE,
    },
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
        mechanism: crate::pointer_offer::PointerMechanism::UsbHid,
        controller_id,
        device_id,
        interface_id,
        endpoint_id,
        report_buffers: u16::from(ready.report_buffers),
        event_slots: crate::pointer_offer::POINTER_EVENT_SLOTS,
        operation_slots: crate::pointer_offer::POINTER_OPERATION_SLOTS,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    identities: &BootIdentities,
    fabrication: &FabricationRecord,
    tour: &mut TourProduct,
    presenter: &mut TourShellPresenter,
    display: &mut impl crate::display::PixelTarget,
    session: &mut HidPointerSession,
    controller: &mut XhciReady,
    usb: &UsbDevice,
) -> Result<(), &'static str> {
    run_with(identities, fabrication, tour, presenter, display, || {
        session
            .receive(controller, usb)
            .map_err(|error| error.as_str())
    })
}

pub fn run_ps2(
    identities: &BootIdentities,
    fabrication: &FabricationRecord,
    tour: &mut TourProduct,
    presenter: &mut TourShellPresenter,
    display: &mut impl crate::display::PixelTarget,
    input: &mut crate::arch::Ps2Input,
) -> Result<(), &'static str> {
    run_with(identities, fabrication, tour, presenter, display, || {
        input.receive_pointer().map_err(|error| error.as_str())
    })
}

fn run_with(
    identities: &BootIdentities,
    fabrication: &FabricationRecord,
    tour: &mut TourProduct,
    presenter: &mut TourShellPresenter,
    display: &mut impl crate::display::PixelTarget,
    mut receive: impl FnMut() -> Result<conduit_semantic_catalog::NormalizedPointerSample, &'static str>,
) -> Result<(), &'static str> {
    arch::early_write(b"CONDUIT_BOOT_STAGE pointer-awaiting-report\n");
    let mut suppress_dismissal_release = false;
    loop {
        let sample = receive()?;
        let format = display
            .format()
            .validate()
            .map_err(crate::display::DisplayError::as_str)?;
        let display_x = display_coordinate(sample.position_x, format.width)?;
        let display_y = display_coordinate(sample.position_y, format.height)?;
        let route = presenter
            .route_pointer(display_x, display_y, sample.primary_pressed)
            .map_err(|error| error.as_str())?;
        let crate::native_compositor::InputRoute::Delivered(route) = route else {
            return Err("compositor-pointer-no-target");
        };
        presenter
            .validate_pointer_route(&route)
            .map_err(|error| error.as_str())?;
        if suppress_dismissal_release && !sample.primary_pressed {
            suppress_dismissal_release = false;
            arch::early_write(b"CONDUIT_BOOT_STAGE pointer-awaiting-report\n");
            continue;
        }
        if route.surface_id != WORKSPACE_SURFACE {
            let close_hit = presenter
                .inspector_close_hit(&route)
                .map_err(|error| error.as_str())?;
            if sample.primary_pressed && close_hit {
                presenter
                    .activate_inspector_close(&route, tour, display)
                    .map_err(|error| error.as_str())?;
                suppress_dismissal_release = true;
                arch::early_write(b"CONDUIT_TOUR_CHECKPOINT inspector-close-activated\n");
                arch::early_write(b"CONDUIT_BOOT_STAGE pointer-awaiting-report\n");
                continue;
            }
            let hovered = close_hit
                || presenter
                    .scroll_hit_subject(&route)
                    .map_err(|error| error.as_str())?
                    .is_some();
            presenter
                .set_pointer_hover(hovered)
                .map_err(|error| error.as_str())?;
            if sample.primary_pressed {
                if route.surface_id != TRANSIENT_SURFACE {
                    presenter
                        .compose_affordances(display)
                        .map_err(|error| error.as_str())?;
                }
                if presenter
                    .scroll_hit_subject(&route)
                    .map_err(|error| error.as_str())?
                    .is_some()
                {
                    arch::early_write(b"CONDUIT_TOUR_CHECKPOINT scrolled-surface-subject-hit\n");
                }
                emit_auxiliary_focus_sign(&route, sample, tour, identities, fabrication);
                arch::early_write(b"CONDUIT_TOUR_CHECKPOINT auxiliary-surface-focused\n");
                if route.surface_id == TRANSIENT_SURFACE {
                    let chosen = presenter
                        .chooser_gear(&route)
                        .map_err(|error| error.as_str())?;
                    if let Some(gear) = chosen {
                        tour.select_gear(tour.controller().state().revision, gear)
                            .map_err(|error| error.as_str())?;
                    }
                    let dismissal = presenter
                        .dismiss_transient(display)
                        .map_err(|error| error.as_str())?;
                    let stale_input_refused = matches!(
                        presenter.validate_pointer_route(&route),
                        Err(TourShellError::Compositor(
                            NativeCompositorError::StaleSurfaceBinding
                        ))
                    );
                    if !stale_input_refused {
                        return Err("dismissed-transient-route-remained-current");
                    }
                    crate::product_front_door::transient_sign::emit_dismissed_transient(
                        &dismissal,
                        true,
                        identities,
                        fabrication,
                    );
                    if chosen.is_some() {
                        presenter
                            .present(tour, display)
                            .map_err(|error| error.as_str())?;
                        arch::early_write(b"CONDUIT_TOUR_CHECKPOINT chooser-gear-selected\n");
                    }
                    suppress_dismissal_release = true;
                    arch::early_write(b"CONDUIT_TOUR_CHECKPOINT transient-pointer-dismissed\n");
                }
            }
            if !sample.primary_pressed && route.surface_id != TRANSIENT_SURFACE {
                presenter
                    .compose_affordances(display)
                    .map_err(|error| error.as_str())?;
            }
            arch::early_write(b"CONDUIT_BOOT_STAGE pointer-awaiting-report\n");
            continue;
        }
        let mut local_sample = sample;
        local_sample.position_x = normalized_local(route.local_x, format.width)?;
        local_sample.position_y = normalized_local(route.local_y, format.height)?;
        let outcome = tour.accept_pointer(
            local_sample,
            u16::try_from(format.width).map_err(|_| "tour-display-extent-invalid")?,
            u16::try_from(format.height).map_err(|_| "tour-display-extent-invalid")?,
        )?;
        presenter
            .set_pointer_hover(true)
            .map_err(|error| error.as_str())?;
        let mut shell = presenter
            .present(tour, display)
            .map_err(|error| error.as_str())?;
        if !sample.primary_pressed && shell.inspector.is_some() {
            let relayout = presenter
                .relayout_inspector(tour, display)
                .map_err(|error| error.as_str())?;
            emit_relayout_sign(&relayout, identities, fabrication);
            shell.inspector = Some(relayout.current);
            shell.frame = relayout.frame;
            arch::early_write(b"CONDUIT_TOUR_CHECKPOINT inspector-relayout\n");
        }
        emit_sign(
            &outcome,
            &route,
            &shell,
            sample,
            tour,
            identities,
            fabrication,
        );
        arch::early_write(b"CONDUIT_BOOT_STAGE pointer-awaiting-report\n");
    }
}

fn normalized_local(value: u16, extent: u32) -> Result<i64, &'static str> {
    if extent == 0 || u32::from(value) >= extent {
        return Err("compositor-pointer-local-coordinate-invalid");
    }
    i64::try_from(u64::from(value) * 1_000_000 / u64::from(extent))
        .map_err(|_| "compositor-pointer-local-coordinate-invalid")
}

fn display_coordinate(value: i64, extent: u32) -> Result<u32, &'static str> {
    if !(0..=1_000_000).contains(&value) || extent == 0 {
        return Err("compositor-pointer-coordinate-invalid");
    }
    let coordinate = u64::try_from(value)
        .ok()
        .and_then(|value| value.checked_mul(u64::from(extent)))
        .map(|value| value / 1_000_000)
        .ok_or("compositor-pointer-coordinate-invalid")?;
    u32::try_from(coordinate.min(u64::from(extent - 1)))
        .map_err(|_| "compositor-pointer-coordinate-invalid")
}

fn emit_sign(
    outcome: &TourPointerOutcome,
    route: &RoutedPointer,
    shell: &ShellPresentationReceipt,
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
        "CONDUIT_POINTER_SIGN {{\"schema\":\"conduit.conduitos.pointer-interaction/v1\",\"status\":\"{status}\",\"subject\":\"{subject}\",\"sequence\":{},\"position_x\":{},\"position_y\":{},\"delta_x\":{},\"delta_y\":{},\"primary_pressed\":{},\"queue_capacity\":{},\"revision\":{},\"routed_surface_id\":\"{}\",\"routed_manifestation_id\":\"{}\",\"local_x\":{},\"local_y\":{},\"workspace_presentation_id\":\"{}\",\"workspace_manifestation_id\":\"{}\",\"inspector_surface_id\":{},\"inspector_presentation_id\":{},\"inspector_manifestation_id\":{},\"frame_sequence\":{},\"surfaces_composed\":{},\"damage_count\":{},\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"proof_class\":\"freestanding-emulator\",\"bounded\":true}}\n",
        sample.sequence,
        sample.position_x,
        sample.position_y,
        sample.delta_x,
        sample.delta_y,
        sample.primary_pressed,
        sample.queue_capacity,
        tour.controller().state().revision,
        route.surface_id,
        route.manifestation_id.as_str(),
        route.local_x,
        route.local_y,
        shell.workspace.presentation_id.as_str(),
        shell.workspace.manifestation_id.as_str(),
        json_optional(
            shell
                .inspector
                .as_ref()
                .map(|value| value.surface_id.as_str())
        ),
        json_optional(
            shell
                .inspector
                .as_ref()
                .map(|value| value.presentation_id.as_str())
        ),
        json_optional(
            shell
                .inspector
                .as_ref()
                .map(|value| value.manifestation_id.as_str())
        ),
        shell.frame.frame_sequence,
        shell.frame.surfaces_composed,
        shell.frame.damage_count,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        identity::hex(&identities.host),
        identity::hex(&identities.boot),
    );
    arch::early_write(line.as_bytes());
}

fn emit_auxiliary_focus_sign(
    route: &RoutedPointer,
    sample: conduit_semantic_catalog::NormalizedPointerSample,
    tour: &TourProduct,
    identities: &BootIdentities,
    fabrication: &FabricationRecord,
) {
    let status = if route.surface_id == TRANSIENT_SURFACE {
        "transient-focused"
    } else {
        "auxiliary-focused"
    };
    let line = format!(
        "CONDUIT_POINTER_SIGN {{\"schema\":\"conduit.conduitos.pointer-interaction/v1\",\"status\":\"{status}\",\"subject\":null,\"sequence\":{},\"position_x\":{},\"position_y\":{},\"delta_x\":{},\"delta_y\":{},\"primary_pressed\":true,\"queue_capacity\":{},\"revision\":{},\"routed_surface_id\":\"{}\",\"routed_manifestation_id\":\"{}\",\"local_x\":{},\"local_y\":{},\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"proof_class\":\"freestanding-emulator\",\"bounded\":true}}\n",
        sample.sequence,
        sample.position_x,
        sample.position_y,
        sample.delta_x,
        sample.delta_y,
        sample.queue_capacity,
        tour.controller().state().revision,
        route.surface_id,
        route.manifestation_id.as_str(),
        route.local_x,
        route.local_y,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        identity::hex(&identities.host),
        identity::hex(&identities.boot),
    );
    arch::early_write(line.as_bytes());
}

fn json_optional(value: Option<&str>) -> alloc::string::String {
    value.map_or_else(|| "null".into(), |value| format!("\"{value}\""))
}

fn emit_relayout_sign(
    receipt: &crate::tour_shell::ShellRelayoutReceipt,
    identities: &BootIdentities,
    fabrication: &FabricationRecord,
) {
    let line = format!(
        "CONDUIT_RESIZE_SIGN {{\"schema\":\"conduit.conduitos.surface-relayout/v1\",\"status\":\"current\",\"surface_id\":\"{}\",\"previous_x\":{},\"previous_y\":{},\"previous_width\":{},\"previous_height\":{},\"current_x\":{},\"current_y\":{},\"current_width\":{},\"current_height\":{},\"invalidated_manifestation_id\":\"{}\",\"current_presentation_id\":\"{}\",\"current_manifestation_id\":\"{}\",\"input_refused_while_invalidated\":{},\"frame_sequence\":{},\"damage_count\":{},\"pixels_written\":{},\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"bounded\":true}}\n",
        receipt.surface_id,
        receipt.previous_bounds.x,
        receipt.previous_bounds.y,
        receipt.previous_bounds.width,
        receipt.previous_bounds.height,
        receipt.current_bounds.x,
        receipt.current_bounds.y,
        receipt.current_bounds.width,
        receipt.current_bounds.height,
        receipt.invalidated_manifestation_id.as_str(),
        receipt.current.presentation_id.as_str(),
        receipt.current.manifestation_id.as_str(),
        receipt.input_refused_while_invalidated,
        receipt.frame.frame_sequence,
        receipt.frame.damage_count,
        receipt.frame.pixels_written,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        identity::hex(&identities.host),
        identity::hex(&identities.boot),
    );
    arch::early_write(line.as_bytes());
}
