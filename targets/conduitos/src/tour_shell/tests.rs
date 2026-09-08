use alloc::{vec, vec::Vec};

use conduit_presentation::{ApplicationEvent, ApplicationEventKind};
use conduit_semantic_catalog::NormalizedPointerSample;
use conduit_tour_model::{OPEN_PATCHBAY_ACTION_ID, TourPointerOutcome, TourTransientKind};

use super::*;
use crate::{
    display::{DisplayError, DisplayFormat},
    identity::BootIdentities,
    machine::{BaseError, IdleBase, InterruptBase, InterruptState, MonotonicClockBase, SerialBase},
    offer::{CpuFeatures, HostOffer},
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
    assert_eq!(transient.surface_id, TRANSIENT_SURFACE);
    assert_eq!(
        delivered(shell.route_pointer(320, 240, true).unwrap()).surface_id,
        TRANSIENT_SURFACE
    );
    assert!(matches!(
        shell.route_keyboard().unwrap(),
        InputRoute::Delivered(_)
    ));
    shell.dismiss_transient(&mut display).unwrap();
    assert_eq!(
        delivered(shell.route_pointer(320, 240, false).unwrap()).surface_id,
        WORKSPACE_SURFACE
    );
}

fn fixture() -> (TourProduct, TourShellPresenter, MemoryDisplay) {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
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
    pixels: Vec<u32>,
}
impl MemoryDisplay {
    fn new() -> Self {
        Self {
            pixels: vec![0; 640 * 480],
        }
    }
}
impl PixelTarget for MemoryDisplay {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 640,
            height: 480,
            pitch: 2560,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        self.pixels[usize::try_from(y * 640 + x).unwrap()] = pixel;
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
