use super::*;
use crate::display::PixelTarget;

fn door() -> FrontDoor {
    FrontDoor::new(
        HostId::from("host"),
        BootId::from("boot"),
        OfferGeneration(3),
        "profile:one",
        "build:one",
        "image:one",
        SourceDocumentId::from("source"),
        CheckedFormId::from("checked"),
        7,
        false,
    )
}

fn key(usage: u8) -> KeyEvent {
    KeyEvent::new(
        usage,
        conduit_human::KeyTransition::Pressed,
        conduit_human::KeyModifiers::from_bits(0),
    )
    .unwrap()
}

struct Sink;

impl PixelTarget for Sink {
    fn format(&self) -> crate::display::DisplayFormat {
        crate::display::DisplayFormat {
            width: 640,
            height: 480,
            pitch: 2_560,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }

    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
        Ok(())
    }
}

#[test]
fn entrance_is_a_zero_body_portable_presentation_and_finite_scene() {
    let door = door();
    let presentation = door.presentation().unwrap();
    assert!(presentation.basis.body_id.is_none());
    assert!(presentation.basis.plan_id.is_none());
    assert_eq!(presentation.subjects[1].role, PresentationRole::Form);
    assert_eq!(presentation.actions.len(), 2);
    assert!(
        presentation
            .actions
            .iter()
            .any(|action| action.intent == "conduit.intent/open@1")
    );
    assert!(presentation.actions.iter().any(|action| {
        action.intent == "conduit.intent/birth@1"
            && matches!(
                action.availability,
                conduit_presentation::PresentationActionAvailability::Unavailable { .. }
            )
    }));
    let scene = door.scene(&Sink).unwrap();
    let receipt = crate::display::render_scene(&mut Sink, &scene).unwrap();
    assert_eq!(receipt.commands, 8);
    assert!(receipt.pixels_written > 0);
}

#[test]
fn open_is_inert_and_details_are_progressive() {
    let mut door = door();
    assert!(door.accept(key(ENTER), 1).unwrap());
    assert!(door.form_open);
    assert_eq!(door.revision(), 2);
    assert!(door.presentation().unwrap().basis.body_id.is_none());
    assert!(door.accept(key(F2), 2).unwrap());
    let details = door.presentation().unwrap();
    assert!(door.exact_details_open());
    assert!(
        details
            .properties
            .iter()
            .any(|property| property.name == "profile-id")
    );
    assert!(details.basis.body_id.is_none());
}

#[test]
fn stale_input_and_capacity_are_explicit_without_body_transition() {
    let mut door = door();
    assert_eq!(door.accept(key(ENTER), 0), Err(Error::StaleInput));
    assert_eq!(
        door.resolve_action(patchbay_control::PatchbayAction::OpenBack, 0),
        Err(Error::StaleAction)
    );
    assert_eq!(
        door.resolve_action(patchbay_control::PatchbayAction::Birth, door.revision()),
        Err(Error::ActionUnavailable)
    );
    door.revision = u64::MAX;
    assert_eq!(door.accept(key(ENTER), u64::MAX), Err(Error::Presentation));
    assert!(door.presentation().unwrap().basis.body_id.is_none());
}

#[test]
fn releases_and_unrelated_keys_do_not_act() {
    let mut door = door();
    let release = KeyEvent::new(
        ENTER,
        conduit_human::KeyTransition::Released,
        conduit_human::KeyModifiers::from_bits(0),
    )
    .unwrap();
    assert!(!door.accept(release, 1).unwrap());
    assert!(!door.accept(key(4), 1).unwrap());
}
