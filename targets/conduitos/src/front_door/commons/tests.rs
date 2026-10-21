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
fn commons_picker_uses_only_current_play_members_and_retains_its_prepared_source() {
    let (ids, offer, mut journey) = fixture();
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
    for action in [
        crate::product_journey::JourneyAction::Wake,
        crate::product_journey::JourneyAction::Plan,
        crate::product_journey::JourneyAction::Play,
    ] {
        crate::product_journey::test_support::invoke(&mut journey, action, &ids, &offer).unwrap();
    }
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
    assert_eq!(door.home_view(), Some(HomeView::Plots));
    assert_eq!(door.commons_place(), Some("commons/hub"));
    assert_eq!(door.home_active_plots().len(), 1);
    let face = door.face().unwrap();
    assert!(
        !face
            .subjects
            .iter()
            .any(|subject| subject.identity == "commons/workshop")
    );
    assert!(
        door.scene(&Sink(1024, 768))
            .unwrap()
            .commands()
            .iter()
            .any(|command| command.payload().contains("Keyboard canvas"))
    );
    let prepared = door.home.as_ref().unwrap().commons.source.borrow().as_ref() as *const Source;
    for _ in 0..4 {
        assert_eq!(
            door.accept_home(key(ENTER, KeyTransition::Pressed), door.revision())
                .unwrap(),
            HomeInput::OpenPlot(0)
        );
        let stale = door.revision();
        door.close_home().unwrap();
        door.open_home().unwrap();
        assert_eq!(
            door.accept_home(key(ENTER, KeyTransition::Pressed), stale),
            Err(Error::StaleInput)
        );
        assert_eq!(
            door.home.as_ref().unwrap().commons.source.borrow().as_ref() as *const Source,
            prepared
        );
    }
    assert_eq!(journey.projection(), original);
    let revision = journey.revision();
    crate::product_journey::test_support::invoke(
        &mut journey,
        crate::product_journey::JourneyAction::Stop,
        &ids,
        &offer,
    )
    .unwrap();
    assert!(journey.revision() > revision);
    door.observe_product(&journey).unwrap();
    assert!(door.home_active_plots().is_empty());
    assert_eq!(
        door.accept_home(key(ENTER, KeyTransition::Pressed), door.revision())
            .unwrap(),
        HomeInput::Unchanged
    );
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
