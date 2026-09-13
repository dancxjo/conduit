//! Input observation receipts remain independent of buffered scanout work.
use super::{PendingInput, refresh, select, tests::listening};
use crate::{
    display::{DisplayError, DisplayFormat, PixelTarget},
    front_door::FrontDoorPresenter,
    product_journey::{ProductJourney, test_support::key},
};
use conduit_core::HostBaseId;
use conduit_human::KeyTransition;

#[derive(Default)]
struct CountingDisplay {
    writes: usize,
}

impl PixelTarget for CountingDisplay {
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

    fn write_pixel(&mut self, _x: u32, _y: u32, _pixel: u32) -> Result<(), DisplayError> {
        self.writes += 1;
        Ok(())
    }
}

fn renderer(journey: &ProductJourney) -> (FrontDoorPresenter, CountingDisplay) {
    let projection = journey.projection();
    let presenter = FrontDoorPresenter::prepare(
        projection.host_id,
        projection.boot_id,
        projection.offer_generation,
        "profile",
        "image",
        HostBaseId::from("display/base"),
        2,
    )
    .unwrap();
    (presenter, CountingDisplay::default())
}

#[test]
fn release_has_an_observation_receipt_without_another_frame() {
    let (mut journey, mut door) = listening();
    let (mut presenter, mut display) = renderer(&journey);
    let mut pending = PendingInput::default();
    pending
        .accept(key(4, KeyTransition::Pressed), &mut journey, &mut door)
        .unwrap();
    let pressed = pending
        .service(&mut door, &journey, &mut presenter, &mut display, true)
        .unwrap()
        .unwrap();
    let before = journey.projection();
    let writes = display.writes;
    assert!(writes > 0);
    assert_eq!(presenter.compositor_frame_sequence(), 1);
    pending
        .accept(key(4, KeyTransition::Released), &mut journey, &mut door)
        .unwrap();
    let released = pending
        .service(&mut door, &journey, &mut presenter, &mut display, true)
        .unwrap()
        .unwrap();
    let after = journey.projection();
    assert_eq!(after.input_count, 2);
    assert_eq!(after.result, before.result);
    assert_eq!(after.active_play_id, before.active_play_id);
    assert_eq!(
        released.presentation_id,
        door.presentation().unwrap().identity
    );
    assert_ne!(released.presentation_id, pressed.presentation_id);
    assert_eq!(display.writes, writes);
    assert_eq!(presenter.compositor_frame_sequence(), 1);
    assert!(
        pending
            .service(&mut door, &journey, &mut presenter, &mut display, true)
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_batched_release_cannot_erase_the_pending_press_repaint() {
    let (mut journey, mut door) = listening();
    let (mut presenter, mut display) = renderer(&journey);
    let mut pending = PendingInput::default();
    for transition in [KeyTransition::Pressed, KeyTransition::Released] {
        pending
            .accept(key(4, transition), &mut journey, &mut door)
            .unwrap();
    }
    assert_eq!(display.writes, 0);
    assert_eq!(presenter.compositor_frame_sequence(), 0);
    assert!(
        pending
            .service(&mut door, &journey, &mut presenter, &mut display, true)
            .unwrap()
            .is_some()
    );
    assert_eq!(journey.projection().input_count, 2);
    assert_eq!(journey.projection().result.as_deref(), Some("A"));
    assert!(display.writes > 0);
    assert_eq!(presenter.compositor_frame_sequence(), 1);
    assert!(
        pending
            .service(&mut door, &journey, &mut presenter, &mut display, true)
            .unwrap()
            .is_none()
    );
}

#[test]
fn held_release_after_selection_is_observed_without_repainting_the_new_form() {
    let (mut journey, mut door) = listening();
    let (mut presenter, mut display) = renderer(&journey);
    let mut pending = PendingInput::default();
    pending
        .accept(key(4, KeyTransition::Pressed), &mut journey, &mut door)
        .unwrap();
    pending
        .service(&mut door, &journey, &mut presenter, &mut display, true)
        .unwrap();
    select(key(43, KeyTransition::Pressed), &mut journey).unwrap();
    refresh(&mut door, &journey, &mut presenter, &mut display).unwrap();
    let frames = presenter.compositor_frame_sequence();
    let writes = display.writes;
    pending
        .accept(key(4, KeyTransition::Released), &mut journey, &mut door)
        .unwrap();
    assert!(
        pending
            .service(&mut door, &journey, &mut presenter, &mut display, true)
            .unwrap()
            .is_some()
    );
    assert_eq!(journey.projection().input_count, 2);
    assert!(journey.projection().result.is_none());
    assert!(!journey.owns_key_release(key(4, KeyTransition::Released)));
    assert_eq!(display.writes, writes);
    assert_eq!(presenter.compositor_frame_sequence(), frames);
}

#[test]
fn hidden_workspace_service_consumes_pending_work_without_touching_scanout() {
    let (mut journey, mut door) = listening();
    let (mut presenter, mut display) = renderer(&journey);
    let mut pending = PendingInput::default();
    pending
        .accept(key(4, KeyTransition::Pressed), &mut journey, &mut door)
        .unwrap();
    assert!(
        pending
            .service(&mut door, &journey, &mut presenter, &mut display, false)
            .unwrap()
            .is_none()
    );
    assert!(
        pending
            .service(&mut door, &journey, &mut presenter, &mut display, true)
            .unwrap()
            .is_none()
    );
    assert_eq!(display.writes, 0);
    assert_eq!(presenter.compositor_frame_sequence(), 0);
}

#[test]
fn unaccepted_input_does_not_fabricate_an_observation() {
    let (journey, mut door) = listening();
    let (mut presenter, mut display) = renderer(&journey);
    let projection = journey.projection();
    let mut dormant = ProductJourney::new(
        projection.host_id,
        projection.boot_id,
        projection.offer_generation,
    )
    .unwrap();
    let mut pending = PendingInput::default();
    pending
        .accept(key(4, KeyTransition::Pressed), &mut dormant, &mut door)
        .unwrap();
    assert!(
        pending
            .service(&mut door, &dormant, &mut presenter, &mut display, true)
            .unwrap()
            .is_none()
    );
    assert_eq!(dormant.projection().input_count, 0);
    assert_eq!(display.writes, 0);
}
