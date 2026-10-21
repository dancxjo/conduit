//! Actual native owner projection and Source navigation, without scanout claims.
use super::*;
use crate::{
    display::{DisplayError, DisplayFormat, PixelTarget},
    native_workset::{self, NativePlot},
    product_journey::test_support::{fixture, key},
};
use conduit_human::KeyTransition;

struct Sink(u32, u32);
impl PixelTarget for Sink {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: self.0,
            height: self.1,
            pitch: self.0 * 4,
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
fn commons_navigation_retains_the_actual_body_and_uses_its_existing_launcher() {
    let (_, _, mut journey) = fixture();
    journey
        .birth_from_creche(conduit_birth_plot::BirthSelection {
            revision: 1,
            friendly_name: "Commons Body".into(),
            workset: conduit_body::BodyWorkset::one(
                native_workset::resident(NativePlot::KeyboardCanvas).unwrap(),
            )
            .unwrap(),
        })
        .unwrap();
    let original = journey.projection();
    let mut door = FrontDoor::new(
        original.host_id.clone(),
        original.boot_id.clone(),
        original.offer_generation,
        "profile",
        "build",
        "image",
        original.source_document_id.clone().unwrap(),
        original.checked_plot_id.clone().unwrap(),
        7,
        true,
    );
    door.observe_product(&journey).unwrap();
    door.open_home().unwrap();
    let hub = door.face().unwrap();
    let selected = |face: &Face, place: &str| {
        face.properties.iter().any(|property| {
            property.subject == place
                && property.name == "selected"
                && property.value == FacePropertyValue::Flag(true)
        })
    };
    assert!(selected(&hub, "commons/hub"));
    assert_eq!(hub.relationships.iter().filter(|relation| matches!(&relation.kind, FaceRelationshipKind::Semantic(kind) if kind.as_str() == "commons/reaches-place")).count(), 2);
    assert!(
        door.scene(&Sink(1280, 800))
            .unwrap()
            .commands()
            .iter()
            .any(|command| command.payload() == "Workshop")
    );
    let narrow = door.scene(&Sink(480, 360)).unwrap();
    assert!(
        narrow
            .commands()
            .iter()
            .any(|command| command.payload().contains("> Workshop"))
    );
    let old_revision = door.revision();
    door.accept_home(key(ENTER, KeyTransition::Pressed), old_revision)
        .unwrap();
    let workshop = door.face().unwrap();
    assert!(selected(&workshop, "commons/workshop"));
    assert_eq!(workshop.basis, hub.basis);
    assert_ne!(workshop.identity, hub.identity);
    assert_eq!(
        door.accept_home(key(ENTER, KeyTransition::Pressed), old_revision),
        Err(Error::StaleInput)
    );
    // Workshop reuses Home's installed Plot selection; it does not birth or run another Body.
    assert_eq!(
        door.accept_home(key(ENTER, KeyTransition::Pressed), door.revision())
            .unwrap(),
        HomeInput::Changed
    );
    door.accept_home(key(ESCAPE, KeyTransition::Pressed), door.revision())
        .unwrap();
    assert!(selected(&door.face().unwrap(), "commons/hub"));
    door.accept_home(key(RIGHT, KeyTransition::Pressed), door.revision())
        .unwrap();
    door.accept_home(key(ENTER, KeyTransition::Pressed), door.revision())
        .unwrap();
    let creche = door.face().unwrap();
    assert!(selected(&creche, "commons/creche"));
    assert!(
        creche
            .actions
            .iter()
            .all(|action| !action.availability.is_available())
    );
    assert!(
        door.scene(&Sink(1280, 800))
            .unwrap()
            .commands()
            .iter()
            .any(|command| command.payload() == "This Body is already born.")
    );
    assert_eq!(journey.projection(), original);
    door.accept_home(key(ESCAPE, KeyTransition::Pressed), door.revision())
        .unwrap();
    assert!(selected(&door.face().unwrap(), "commons/hub"));
    door.accept_home(key(LEFT, KeyTransition::Pressed), door.revision())
        .unwrap();
    door.accept_home(key(ENTER, KeyTransition::Pressed), door.revision())
        .unwrap();
    let prepared = door.home.as_ref().unwrap().commons.source.borrow().as_ref() as *const Source;
    for _ in 0..4 {
        door.close_home().unwrap();
        door.open_home().unwrap();
        assert!(selected(&door.face().unwrap(), "commons/hub"));
        assert_eq!(
            door.home.as_ref().unwrap().commons.source.borrow().as_ref() as *const Source,
            prepared
        );
        door.accept_home(key(LEFT, KeyTransition::Pressed), door.revision())
            .unwrap();
        door.accept_home(key(ENTER, KeyTransition::Pressed), door.revision())
            .unwrap();
    }
    assert_eq!(journey.projection(), original);
}

#[test]
fn zero_body_commons_returns_to_the_same_creche_draft_without_lifecycle_effects() {
    let (_, _, journey) = fixture();
    let original = journey.projection();
    let plot = native_workset::resident(NativePlot::KeyboardCanvas).unwrap();
    let mut door = FrontDoor::new(
        original.host_id.clone(),
        original.boot_id.clone(),
        original.offer_generation,
        "profile",
        "build",
        "image",
        plot.source_document_id,
        plot.checked_plot_id,
        7,
        true,
    );
    door.observe_product(&journey).unwrap();
    door.open_creche("00000000-0000-0000-0000-000000000001".into(), None)
        .unwrap();
    door.open_home().unwrap();
    assert!(!door.creche_open());
    assert_eq!(door.commons_place(), Some("commons/hub"));
    assert!(
        door.face()
            .unwrap()
            .actions
            .iter()
            .all(|action| !action.availability.is_available())
    );
    door.accept_home(key(RIGHT, KeyTransition::Pressed), door.revision())
        .unwrap();
    assert_eq!(
        door.accept_home(key(ENTER, KeyTransition::Pressed), door.revision())
            .unwrap(),
        HomeInput::OpenCreche
    );
    door.enter_prebirth_creche().unwrap();
    assert!(door.creche_open());
    door.accept_creche(key(4, KeyTransition::Pressed), door.revision())
        .unwrap();
    let draft = door.face().unwrap();
    let stale = door.revision();
    door.open_home().unwrap();
    assert_eq!(door.commons_place(), Some("commons/hub"));
    assert!(!door.creche_open());
    assert_eq!(
        door.accept_home(key(ENTER, KeyTransition::Pressed), stale),
        Err(Error::StaleInput)
    );
    assert_eq!(
        door.accept_home(key(ENTER, KeyTransition::Pressed), door.revision())
            .unwrap(),
        HomeInput::OpenCreche
    );
    door.enter_prebirth_creche().unwrap();
    let returned = door.face().unwrap();
    assert_eq!(returned.properties, draft.properties);
    assert_eq!(returned.actions, draft.actions);
    assert_eq!(journey.projection(), original);
}
