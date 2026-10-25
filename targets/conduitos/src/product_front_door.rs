//! Long-lived ordinary product service for the Patchbay lifecycle journey.

mod application_request;
mod arrival;
mod controller_io;
mod controller_loop;
mod face_arrival;
mod face_workspace;
mod guest_input;
mod guest_join;
mod home_input;
mod input_actions;
mod line_session;
mod platform;
mod pointer_input;
mod prebirth;
mod runtime;
mod startup;
pub use platform::run;
#[cfg(test)]
pub(crate) use startup::tests::Encounter as StartupEncounter;
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
        {
            let mut observed = owner.borrow_mut();
            observed.bind_lifecycle_make(*make);
            observed.emit_prepared_lifecycle();
        }
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
        arrival::open(&mut front_door, &mut journey, identities, offer, make)?;
        None
    };
    let owner_route = owner_face
        .as_ref()
        .filter(|face| face.interactions_admitted())
        .and(owner_route);
    let current_owner_face = owner_face.clone();
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
            // Explicitly admit the installed checked-Source inventory before Play.
            // The Standard Mask and scene/surface limits remain unchanged.
            conduit_conduitos_mask_offer::NativeMaskStorageProfile::Collection,
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
    let owner_route_standby = owner_route.as_ref().is_some_and(|route| route.available());
    let receipt = Progress::result(Phase::FirstShow, "arrival", || {
        if let Some(owner_face) = owner_face {
            face_arrival.present_owner_face(owner_face.into_face(), false, display)
        } else if provisioned_guest {
            face_arrival.present_pending_join(&front_door, display)
        } else if journey.projection().body_id.is_some() {
            presenter
                .present(&front_door, display)
                .map_err(|error| error.as_str())
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
    let pointer_state = pointer_input::PointerState::new();
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
    if !provisioned_guest && journey.projection().body_id.is_some() {
        startup::after_prepared_boot(
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
    controller_loop::run(
        identities,
        offer,
        make,
        display,
        platform,
        runtime,
        effect_bases,
        journey,
        front_door,
        presenter,
        face_arrival,
        face_workspace,
        pointer_state,
        provisioned_guest,
        pointer_admitted,
        line_available,
        owner_face_presented,
        owner_route_standby,
        owner_route,
        current_owner_face,
    )
}

#[cfg(test)]
mod input_routing_tests;
