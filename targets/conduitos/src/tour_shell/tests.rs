use alloc::{format, vec, vec::Vec};

use conduit_presentation::{ApplicationEvent, ApplicationEventKind, GraphicsClipClass};
use conduit_semantic_catalog::NormalizedPointerSample;
use conduit_tour_model::{OPEN_PATCHBAY_ACTION_ID, TourPointerOutcome, TourTransientKind};

use super::*;
use crate::{
    display::{DisplayError, DisplayFormat},
    identity::BootIdentities,
    keyboard_offer::KeyboardRealization,
    machine::{BaseError, IdleBase, InterruptBase, InterruptState, MonotonicClockBase, SerialBase},
    offer::{CpuFeatures, HostOffer},
    product_journey::{JourneyAction, JourneyError, ProductJourney},
};

#[test]
fn selected_gear_manifests_independent_focused_inspector_and_dismisses_it() {
    let (mut tour, mut shell, mut display) = fixture();
    let initial = shell.present(&tour, &mut display).unwrap();
    assert!(initial.inspector.is_none());
    assert_ne!(
        initial.workspace.presentation_id,
        initial.status.presentation_id
    );

    let outcome = tour.accept_pointer(pointer(), 640, 480).unwrap();
    assert_eq!(
        outcome,
        TourPointerOutcome::Selected {
            subject: "meet-one-gear/change".into()
        }
    );
    let selected = shell.present(&tour, &mut display).unwrap();
    let inspector = selected
        .inspector
        .expect("selected Gear must manifest its inspector");
    assert_eq!(inspector.surface_id, INSPECTOR_SURFACE);
    assert_ne!(
        inspector.presentation_id,
        selected.workspace.presentation_id
    );
    let routed = delivered(shell.route_pointer(500, 100, true).unwrap());
    assert_eq!(routed.surface_id, INSPECTOR_SURFACE);
    assert!(matches!(
        shell.route_keyboard().unwrap(),
        InputRoute::Delivered(RoutedKeyboard { surface_id, .. }) if surface_id == INSPECTOR_SURFACE
    ));

    assert!(tour.dismiss_inspector().unwrap());
    assert!(
        shell
            .present(&tour, &mut display)
            .unwrap()
            .inspector
            .is_none()
    );
    assert_eq!(
        delivered(shell.route_pointer(500, 100, false).unwrap()).surface_id,
        WORKSPACE_SURFACE
    );
}

#[test]
fn transient_is_an_independent_related_surface_and_dismissal_exposes_parent() {
    let (tour, mut shell, mut display) = fixture();
    shell.present(&tour, &mut display).unwrap();
    let transient = shell
        .show_transient(
            &tour,
            TourTransientKind::Refusal,
            "Action refused",
            &mut display,
        )
        .unwrap();
    assert!(shell.has_transient());
    assert_eq!(transient.transient.surface_id, TRANSIENT_SURFACE);
    assert_ne!(
        transient.parent_presentation_id,
        transient.transient.presentation_id
    );
    assert_eq!(
        delivered(shell.route_pointer(320, 240, true).unwrap()).surface_id,
        TRANSIENT_SURFACE
    );
    assert!(matches!(
        shell.route_keyboard().unwrap(),
        InputRoute::Delivered(_)
    ));
    let stale = delivered(shell.route_pointer(320, 240, true).unwrap());
    let dismissal = shell.dismiss_transient(&mut display).unwrap();
    assert!(!shell.has_transient());
    assert_eq!(
        shell.validate_pointer_route(&stale),
        Err(TourShellError::Compositor(
            NativeCompositorError::StaleSurfaceBinding
        ))
    );
    assert_eq!(dismissal.manifestation_id, stale.manifestation_id);
    assert!(matches!(
        shell.route_keyboard().unwrap(),
        InputRoute::Delivered(RoutedKeyboard { surface_id, .. }) if surface_id == WORKSPACE_SURFACE
    ));
    assert_eq!(
        delivered(shell.route_pointer(320, 240, false).unwrap()).surface_id,
        WORKSPACE_SURFACE
    );
}

#[test]
fn chooser_scroll_is_finite_and_off_viewport_rows_are_clipped() {
    let (tour, mut shell, mut display) = fixture();
    shell.present(&tour, &mut display).unwrap();
    shell
        .show_transient(
            &tour,
            TourTransientKind::Chooser,
            "Choose a Patchbay Gear",
            &mut display,
        )
        .unwrap();
    delivered(shell.route_pointer(320, 240, true).unwrap());
    let ScrollOutcome::Updated(receipt) = shell
        .scroll_focused(ScrollDirection::End, &mut display)
        .unwrap()
    else {
        panic!("focused chooser must reach its finite end");
    };
    assert_eq!(receipt.surface_id, TRANSIENT_SURFACE);
    let state = shell
        .surfaces
        .iter()
        .find(|state| state.slot == Slot::Transient)
        .unwrap();
    assert_eq!(state.scroll.offset(), state.scroll.maximum_offset());
    let scene = transient_scene(
        state.bounds.unwrap(),
        state.presentation.as_ref().unwrap(),
        state.scroll.offset(),
    )
    .unwrap();
    assert!(
        scene
            .commands()
            .iter()
            .any(|command| command.clip_class() == GraphicsClipClass::FullyClipped)
    );
    assert_eq!(
        shell
            .scroll_focused(ScrollDirection::End, &mut display)
            .unwrap(),
        ScrollOutcome::Boundary
    );
}

#[test]
fn focused_scrolling_revises_only_that_surface_and_translates_clipped_hits() {
    let (mut tour, mut shell, mut display) = fixture();
    tour.accept_pointer(pointer(), 640, 480).unwrap();
    let presented = shell.present(&tour, &mut display).unwrap();
    let workspace_manifestation = presented.workspace.manifestation_id;
    let before = delivered(shell.route_pointer(500, 80, true).unwrap());
    assert_eq!(shell.scroll_hit_subject(&before).unwrap(), Some(0));

    let ScrollOutcome::Updated(scrolled) = shell
        .scroll_focused(ScrollDirection::Forward, &mut display)
        .unwrap()
    else {
        panic!("focused inspector must scroll");
    };
    assert_eq!(scrolled.surface_id, INSPECTOR_SURFACE);
    assert_eq!(scrolled.previous_offset, 0);
    assert_eq!(scrolled.current_offset, 48);
    assert_eq!(
        shell.validate_pointer_route(&before),
        Err(TourShellError::Compositor(
            NativeCompositorError::StaleSurfaceBinding
        ))
    );
    assert_eq!(
        shell
            .surfaces
            .iter()
            .find(|state| state.slot == Slot::Workspace)
            .unwrap()
            .manifestation_id
            .as_ref(),
        Some(&workspace_manifestation)
    );
    let after = delivered(shell.route_pointer(500, 32, false).unwrap());
    assert_eq!(shell.scroll_hit_subject(&after).unwrap(), Some(0));
    shell.present(&tour, &mut display).unwrap();
    assert_eq!(
        shell
            .surfaces
            .iter()
            .find(|state| state.slot == Slot::Inspector)
            .unwrap()
            .scroll
            .offset(),
        48
    );

    assert!(matches!(
        shell
            .scroll_focused(ScrollDirection::End, &mut display)
            .unwrap(),
        ScrollOutcome::Updated(_)
    ));
    assert_eq!(
        shell
            .scroll_focused(ScrollDirection::End, &mut display)
            .unwrap(),
        ScrollOutcome::Boundary
    );
    let relayout = shell.relayout_inspector(&tour, &mut display).unwrap();
    let inspector = shell
        .surfaces
        .iter()
        .find(|state| state.slot == Slot::Inspector)
        .unwrap();
    assert!(inspector.scroll.offset() <= inspector.scroll.maximum_offset());
    assert_eq!(relayout.current.surface_id, INSPECTOR_SURFACE);
}

#[test]
fn suspending_shell_releases_all_retained_surface_targets() {
    let (tour, mut shell, mut display) = fixture();
    shell.present(&tour, &mut display).unwrap();
    shell
        .show_transient(
            &tour,
            TourTransientKind::Chooser,
            "Choose an action",
            &mut display,
        )
        .unwrap();

    shell.suspend().unwrap();

    assert!(!shell.has_transient());
    assert_eq!(
        shell.route_pointer(10, 10, false).unwrap(),
        InputRoute::NoTarget
    );
    assert_eq!(shell.route_keyboard().unwrap(), InputRoute::NoTarget);
}

#[test]
fn status_surface_uses_exact_body_wake_plan_and_play_basis() {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = host_offer(&identities);
    let mut journey = ProductJourney::new(
        HostId::from(crate::identity::hex(&identities.host)),
        BootId::from(crate::identity::hex(&identities.boot)),
        OfferGeneration(offer.generation),
    )
    .unwrap();
    let (tour, mut shell, mut display) = fixture();
    shell
        .present_with_lifecycle(&tour, &journey.projection(), &mut display)
        .expect("a pre-Birth lifecycle must remain a valid status basis");
    for action in [
        JourneyAction::OpenBack,
        JourneyAction::Birth,
        JourneyAction::Wake,
        JourneyAction::Plan,
        JourneyAction::Play,
    ] {
        invoke_journey(&mut journey, action, &identities, &offer).unwrap();
    }
    let projection = journey.projection();
    let (tour, mut shell, mut display) = fixture();
    let receipt = shell
        .present_with_lifecycle(&tour, &projection, &mut display)
        .unwrap();

    assert_eq!(receipt.status.surface_id, STATUS_SURFACE);
    assert_eq!(shell.lifecycle_revision, projection.revision);
    assert_eq!(shell.lifecycle_basis.body_id, projection.body_id);
    assert_eq!(shell.lifecycle_basis.wake_id, projection.wake_id);
    assert_eq!(shell.lifecycle_basis.plan_id, projection.plan_id);
    assert_eq!(
        shell.lifecycle_basis.active_play_id,
        projection.active_play_id
    );
}

#[test]
fn canonical_live_iso_dimensions_do_not_enter_the_scroll_contract() {
    let (_, mut shell, _) = fixture();
    let tour = TourProduct::canonical(1);
    let mut display = MemoryDisplay::with_dimensions(1280, 800);

    let receipt = shell.present(&tour, &mut display).unwrap();

    assert_eq!(receipt.workspace.surface_id, WORKSPACE_SURFACE);
    assert_eq!(receipt.status.surface_id, STATUS_SURFACE);
}

#[test]
fn status_revision_advances_when_tour_state_changes_under_one_lifecycle_basis() {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = host_offer(&identities);
    let mut journey = ProductJourney::new(
        HostId::from(crate::identity::hex(&identities.host)),
        BootId::from(crate::identity::hex(&identities.boot)),
        OfferGeneration(offer.generation),
    )
    .unwrap();
    for action in [
        JourneyAction::OpenBack,
        JourneyAction::Birth,
        JourneyAction::Wake,
        JourneyAction::Plan,
        JourneyAction::Play,
    ] {
        invoke_journey(&mut journey, action, &identities, &offer).unwrap();
    }
    let lifecycle = journey.projection();
    let (mut tour, mut shell, mut display) = fixture();
    let initial = shell
        .present_with_lifecycle(&tour, &lifecycle, &mut display)
        .unwrap();
    tour.accept_pointer(pointer(), 640, 480).unwrap();
    let updated = shell
        .present_with_lifecycle(&tour, &lifecycle, &mut display)
        .unwrap();
    assert!(updated.status.display.commands > 0);
    assert_ne!(
        initial.status.presentation_id,
        updated.status.presentation_id
    );
}

fn invoke_journey(
    journey: &mut ProductJourney,
    action: JourneyAction,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
) -> Result<(), JourneyError> {
    let revision = journey.revision();
    let projection = journey.projection();
    let target = match action {
        JourneyAction::OpenBack | JourneyAction::Birth => {
            format!("form/{}", projection.checked_form_id.as_str())
        }
        _ => format!("body/{}", projection.body_id.unwrap().as_str()),
    };
    let request = journey.next_request(action, target, revision)?;
    journey.apply(request, identities, offer, "build", revision)
}

fn fixture() -> (TourProduct, TourShellPresenter, MemoryDisplay) {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = host_offer(&identities);
    let mut tour = TourProduct::canonical(1);
    tour.accept(
        &ApplicationEvent {
            revision: 1,
            action: OPEN_PATCHBAY_ACTION_ID.into(),
            kind: ApplicationEventKind::Activate,
            value: vec![],
        },
        &identities,
        &offer,
        "build",
        &mut Clock::default(),
        &mut Serial::default(),
        &mut Interrupts::default(),
        &mut Idle::default(),
    )
    .unwrap();
    let shell = TourShellPresenter::prepare(
        HostId::from("host"),
        BootId::from("boot"),
        OfferGeneration(1),
        "profile",
        "image",
        HostBaseId::from("display/base"),
        4,
    )
    .unwrap();
    (tour, shell, MemoryDisplay::new())
}

fn host_offer(identities: &BootIdentities) -> HostOffer<'_> {
    HostOffer::new(
        identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        1_048_576,
    )
    .with_keyboard(
        KeyboardRealization {
            controller_id: [3; 32],
            device_id: [4; 32],
            interface_id: [5; 32],
            endpoint_id: [6; 32],
            report_buffers: 2,
            transition_slots: 8,
            operation_slots: 2,
        },
        "build",
    )
    .unwrap()
}

fn pointer() -> NormalizedPointerSample {
    NormalizedPointerSample {
        position_x: 700_000,
        position_y: 100_000,
        delta_x: 0,
        delta_y: 0,
        primary_pressed: true,
        coalesced: 0,
        dropped: 0,
        queue_capacity: 2,
        sequence: 1,
    }
}

fn delivered<T>(route: InputRoute<T>) -> T {
    match route {
        InputRoute::Delivered(value) => value,
        InputRoute::NoTarget => panic!("expected routed input"),
    }
}

struct MemoryDisplay {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}
impl MemoryDisplay {
    fn new() -> Self {
        Self::with_dimensions(640, 480)
    }

    fn with_dimensions(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; usize::try_from(width * height).unwrap()],
        }
    }
}
impl PixelTarget for MemoryDisplay {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: self.width,
            height: self.height,
            pitch: self.width * 4,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        self.pixels[usize::try_from(y * self.width + x).unwrap()] = pixel;
        Ok(())
    }
}
#[derive(Default)]
struct Clock(u64);
impl MonotonicClockBase for Clock {
    fn now(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}
#[derive(Default)]
struct Serial(Vec<Vec<u8>>);
impl SerialBase for Serial {
    fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
        self.0.push(bytes.into());
        Ok(())
    }
    fn presentation_count(&self) -> u32 {
        self.0.len() as u32
    }
}
#[derive(Default)]
struct Interrupts(bool);
impl InterruptBase for Interrupts {
    fn enable(&mut self) {
        self.0 = true;
    }
    fn disable(&mut self) -> InterruptState {
        let state = InterruptState { enabled: self.0 };
        self.0 = false;
        state
    }
    fn restore(&mut self, state: InterruptState) {
        self.0 = state.enabled;
    }
    fn is_enabled(&self) -> bool {
        self.0
    }
}
#[derive(Default)]
struct Idle(u32);
impl IdleBase for Idle {
    fn wait_for_interrupt(&mut self) -> Result<(), BaseError> {
        self.0 += 1;
        Ok(())
    }
    fn idle_count(&self) -> u32 {
        self.0
    }
}
