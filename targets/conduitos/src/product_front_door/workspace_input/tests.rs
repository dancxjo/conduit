use super::*;
use crate::{
    native_workset,
    product_journey::{
        JourneyAction,
        test_support::{fixture, invoke, key},
    },
};
use conduit_body::BodyWorkset;
use conduit_creche_model::birth::BirthSelection;
use conduit_presentation::PresentationPropertyValue;

fn listening() -> (ProductJourney, FrontDoor) {
    let (ids, offer, mut journey) = fixture();
    invoke(&mut journey, JourneyAction::OpenBack, &ids, &offer).unwrap();
    journey
        .birth_from_creche(BirthSelection {
            revision: 3,
            friendly_name: "Roseau".into(),
            workset: BodyWorkset::from_forms(
                native_workset::inventory()
                    .into_iter()
                    .map(|form| native_workset::resident(form).unwrap()),
            )
            .unwrap(),
        })
        .unwrap();
    for action in [
        JourneyAction::Wake,
        JourneyAction::Plan,
        JourneyAction::Play,
    ] {
        invoke(&mut journey, action, &ids, &offer).unwrap();
    }
    let projection = journey.projection();
    let mut door = FrontDoor::new(
        projection.host_id,
        projection.boot_id,
        projection.offer_generation,
        "profile",
        "build",
        "image",
        projection.source_document_id,
        projection.checked_form_id,
        7,
        true,
    );
    door.observe_product(&journey).unwrap();
    (journey, door)
}

#[test]
fn tab_and_captured_release_preserve_two_form_state_and_the_same_play() {
    let (mut journey, mut door) = listening();
    let before = journey.projection();
    let press = key(4, KeyTransition::Pressed);
    let release = key(4, KeyTransition::Released);
    update(press, &mut journey, &mut door, |_| {
        panic!("unexpected refusal")
    })
    .unwrap();
    assert!(journey.owns_key_release(release));
    assert_eq!(
        select(key(43, KeyTransition::Pressed), &mut journey),
        Ok(Some(true))
    );
    let selected = journey.projection();
    assert_ne!(selected.checked_form_id, before.checked_form_id);
    assert_eq!(
        select(key(43, KeyTransition::Released), &mut journey),
        Ok(Some(false))
    );
    assert_eq!(journey.projection(), selected);
    // This route has no dependency on the foreground presenter or its focus.
    update(release, &mut journey, &mut door, |_| {
        panic!("unexpected refusal")
    })
    .unwrap();
    assert!(!journey.owns_key_release(release));
    assert_eq!(journey.projection().input_count, 2);
    assert!(journey.projection().result.is_none());
    select(key(43, KeyTransition::Pressed), &mut journey).unwrap();
    let after = journey.projection();
    assert_eq!(after.result.as_deref(), Some("A"));
    assert_eq!(after.body_id, before.body_id);
    assert_eq!(after.plan_id, before.plan_id);
    assert_eq!(after.active_play_id, before.active_play_id);
}

#[test]
fn actual_memory_bound_stops_play_and_keeps_the_body_refusal_inspectable() {
    let (mut journey, mut door) = listening();
    journey
        .select_form(
            &native_workset::resident(native_workset::NativeForm::MemoryLantern).unwrap(),
            journey.revision(),
        )
        .unwrap();
    let before = journey.projection();
    for _ in 0..256 {
        for transition in [KeyTransition::Pressed, KeyTransition::Released] {
            update(key(4, transition), &mut journey, &mut door, |_| {
                panic!("within admitted limit")
            })
            .unwrap();
        }
    }
    let mut refusal = None;
    assert!(
        update(
            key(4, KeyTransition::Pressed),
            &mut journey,
            &mut door,
            |reason| refusal = Some(alloc::string::String::from(reason))
        )
        .unwrap()
    );
    assert!(refusal.is_some());
    let after = journey.projection();
    assert_eq!(after.status, JourneyStatus::Stopped);
    assert_eq!(after.body_id, before.body_id);
    assert_eq!(after.active_play_id, before.active_play_id);
    assert_eq!(after.result.as_ref().unwrap().len(), 256);
    assert!(
        door.presentation()
            .unwrap()
            .properties
            .iter()
            .any(|property| {
                property.name == "play-refusal"
                    && property.value == PresentationPropertyValue::Text(refusal.clone().unwrap())
            })
    );
    assert!(!journey.owns_key_release(key(4, KeyTransition::Released)));
}
