use super::test_support::{fixture, invoke, key};
use super::*;
use conduit_body::BodyWorkset;
use conduit_creche_model::birth::BirthSelection;
use conduit_human::KeyTransition;

fn born() -> (BootIdentities, HostOffer<'static>, ProductJourney) {
    let (ids, offer, mut journey) = fixture();
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
    (ids, offer, journey)
}
fn run(journey: &mut ProductJourney, ids: &BootIdentities, offer: &HostOffer<'_>) {
    for action in [
        JourneyAction::Wake,
        JourneyAction::Plan,
        JourneyAction::Play,
    ] {
        invoke(journey, action, ids, offer).unwrap();
    }
}
fn type_key(journey: &mut ProductJourney, usage: u8) {
    for transition in [KeyTransition::Pressed, KeyTransition::Released] {
        assert!(journey.accept_play_input(key(usage, transition)).unwrap());
    }
}
fn select(journey: &mut ProductJourney, form: NativeForm) {
    journey
        .select_form(&native_workset::resident(form).unwrap(), journey.revision())
        .unwrap();
}

#[test]
fn native_birth_keeps_two_forms_in_one_body_plan_play_and_switches_only_foreground() {
    let (ids, offer, mut journey) = born();
    let born = journey.workspace_projection().unwrap();
    assert_eq!(born.forms.len(), 2);
    run(&mut journey, &ids, &offer);
    let started = journey.projection();
    assert_eq!(started.gear_ids.len(), 8);
    assert_eq!(started.cord_ids.len(), 6);
    select(&mut journey, NativeForm::KeyboardCanvas);
    type_key(&mut journey, 4);
    assert_eq!(journey.projection().result.as_deref(), Some("A"));
    select(&mut journey, NativeForm::MemoryLantern);
    type_key(&mut journey, 5);
    type_key(&mut journey, 6);
    assert_eq!(journey.projection().result.as_deref(), Some("bc"));
    select(&mut journey, NativeForm::KeyboardCanvas);
    type_key(&mut journey, 7);
    assert_eq!(journey.projection().result.as_deref(), Some("AD"));
    select(&mut journey, NativeForm::MemoryLantern);
    type_key(&mut journey, 42);
    assert_eq!(journey.projection().result.as_deref(), Some("b"));
    let final_view = journey.projection();
    assert_eq!(final_view.body_id, started.body_id);
    assert_eq!(final_view.plan_id, started.plan_id);
    assert_eq!(final_view.active_play_id, started.active_play_id);
    assert_eq!(final_view.input_count, 10);
    let current = journey.workspace_projection().unwrap();
    assert_eq!(
        current.forms.iter().filter(|form| form.foreground).count(),
        1
    );
    let before = journey.projection();
    assert_eq!(
        journey.select_next_form(0),
        Err(JourneyError::StalePresentation)
    );
    assert_eq!(journey.projection(), before);
}

#[test]
fn lull_retains_both_forms_and_next_wake_prepares_fresh_plan_play() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    let first = journey.projection();
    type_key(&mut journey, 4);
    invoke(&mut journey, JourneyAction::Stop, &ids, &offer).unwrap();
    invoke(&mut journey, JourneyAction::Lull, &ids, &offer).unwrap();
    assert_eq!(journey.workspace_projection().unwrap().forms.len(), 2);
    assert!(
        !journey
            .accept_play_input(key(5, KeyTransition::Pressed))
            .unwrap()
    );
    run(&mut journey, &ids, &offer);
    let next = journey.projection();
    assert_eq!(next.body_id, first.body_id);
    assert_ne!(next.wake_id, first.wake_id);
    assert_ne!(next.plan_id, first.plan_id);
    assert_ne!(next.active_play_id, first.active_play_id);
    assert_eq!(next.input_count, 0);
    type_key(&mut journey, 5);
    assert!(journey.projection().result.is_some());
}

#[test]
fn one_lull_action_retires_a_listening_body_and_preserves_its_workset() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    let before = journey.workspace_projection().unwrap();
    journey
        .accept_play_input(key(4, KeyTransition::Pressed))
        .unwrap();
    invoke(&mut journey, JourneyAction::Lull, &ids, &offer).unwrap();
    assert_eq!(journey.status(), JourneyStatus::Lulled);
    assert!(journey.kernel.is_none());
    assert!(
        !journey
            .accept_play_input(key(4, KeyTransition::Released))
            .unwrap()
    );
    assert_eq!(journey.workspace_projection().unwrap().forms, before.forms);
    assert_eq!(
        journey.workspace_projection().unwrap().body_id,
        before.body_id
    );
}

#[test]
fn foreground_selection_before_admission_does_not_invent_a_plan_or_play() {
    let (ids, offer, mut journey) = born();
    let before = journey.projection();
    select(&mut journey, NativeForm::MemoryLantern);
    let selected = journey.projection();
    assert_eq!(selected.status, JourneyStatus::BornLulled);
    assert_eq!(selected.body_id, before.body_id);
    assert!(selected.plan_id.is_none() && selected.active_play_id.is_none());
    invoke(&mut journey, JourneyAction::Wake, &ids, &offer).unwrap();
    select(&mut journey, NativeForm::KeyboardCanvas);
    let waiting = journey.projection();
    assert_eq!(waiting.status, JourneyStatus::Awake);
    assert!(waiting.plan_id.is_none() && waiting.active_play_id.is_none());
}
