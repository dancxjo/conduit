use alloc::{vec, vec::Vec};

use conduit_core::{BootId, CheckedFormId, HostBaseId, HostId, OfferGeneration, SourceDocumentId};
use conduit_presentation::{ApplicationEvent, ApplicationEventKind};
use conduit_semantic_catalog::NormalizedPointerSample;
use conduit_tour_model::{OPEN_PATCHBAY_ACTION_ID, RUN_ACTION_ID, TourPointerOutcome};

use super::*;
use crate::{
    display::{DisplayError, DisplayFormat, PixelTarget},
    machine::{BaseError, IdleBase, InterruptBase, InterruptState, MonotonicClockBase, SerialBase},
    native_compositor::InputRoute,
    offer::{CpuFeatures, HostOffer},
};

#[test]
fn native_pointer_and_keyboard_cross_surface_routing_before_typed_tour_interaction() {
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
    let mut presenter = FrontDoorPresenter::prepare(
        HostId::from(identity::hex(&identities.host)),
        BootId::from(identity::hex(&identities.boot)),
        OfferGeneration(1),
        "profile",
        "image",
        HostBaseId::from("display/base"),
        1,
    )
    .unwrap();
    let door = FrontDoor::new(
        HostId::from(identity::hex(&identities.host)),
        BootId::from(identity::hex(&identities.boot)),
        OfferGeneration(1),
        "profile",
        "build",
        "image",
        SourceDocumentId::from("source"),
        CheckedFormId::from("checked"),
        1,
        true,
    );
    let mut display = MemoryDisplay::new();
    presenter.present(&door, &mut display).unwrap();

    let pointer = match presenter.route_pointer(448, 48, true).unwrap() {
        InputRoute::Delivered(route) => route,
        InputRoute::NoTarget => panic!("native Patchbay surface was not routed"),
    };
    presenter.validate_pointer_route(&pointer).unwrap();
    assert_eq!((pointer.local_x, pointer.local_y), (448, 48));

    let mut tour = TourProduct::canonical(3);
    let mut clock = Clock::default();
    let mut serial = Serial::default();
    let mut interrupts = Interrupts::default();
    let mut idle = Idle::default();
    tour.accept(
        &event(3, OPEN_PATCHBAY_ACTION_ID),
        &identities,
        &offer,
        "build",
        &mut clock,
        &mut serial,
        &mut interrupts,
        &mut idle,
    )
    .unwrap();
    let selected = tour
        .accept_pointer(
            NormalizedPointerSample {
                position_x: i64::from(pointer.local_x) * 1_000_000 / 640,
                position_y: i64::from(pointer.local_y) * 1_000_000 / 480,
                delta_x: 0,
                delta_y: 0,
                primary_pressed: true,
                coalesced: 0,
                dropped: 0,
                queue_capacity: 2,
                sequence: 1,
            },
            640,
            480,
        )
        .unwrap();
    assert_eq!(
        selected,
        TourPointerOutcome::Selected {
            subject: "meet-one-gear/change".into()
        }
    );

    assert!(matches!(
        presenter.route_keyboard().unwrap(),
        InputRoute::Delivered(_)
    ));
    let update = tour
        .accept(
            &event(tour.controller().state().revision, RUN_ACTION_ID),
            &identities,
            &offer,
            "build",
            &mut clock,
            &mut serial,
            &mut interrupts,
            &mut idle,
        )
        .unwrap();
    let play = update
        .play
        .expect("keyboard invocation must complete an admitted Play");
    assert!(!play.plan_id.as_str().is_empty());
    assert!(!play.active_play_id.as_str().is_empty());
    assert_eq!(serial.0, [conduit_tour_model::CANONICAL_RESULT.as_bytes()]);
}

fn event(revision: u32, action: &str) -> ApplicationEvent {
    ApplicationEvent {
        revision,
        action: action.into(),
        kind: ApplicationEventKind::Activate,
        value: vec![],
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
            pitch: 2_560,
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
