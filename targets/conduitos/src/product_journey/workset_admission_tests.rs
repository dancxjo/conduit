use super::super::test_support::{fixture, invoke, key};
use super::*;
use conduit_birth_plot::BirthSelection;
use conduit_body::BodyWorkset;

#[test]
fn canonical_admission_before_foreground_preserves_identity_result_and_presentation() {
    let (ids, offer, mut journey) = fixture();
    let memory = native_workset::resident(NativePlot::MemoryLantern).unwrap();
    let keyboard = native_workset::resident(NativePlot::KeyboardCanvas).unwrap();
    assert!(
        keyboard < memory,
        "regression needs insertion before the existing resident"
    );
    journey
        .birth_from_creche(BirthSelection {
            revision: 1,
            friendly_name: "Ordered Body".into(),
            workset: BodyWorkset::one(memory.clone()).unwrap(),
        })
        .unwrap();
    for action in [
        JourneyAction::Wake,
        JourneyAction::Plan,
        JourneyAction::Play,
    ] {
        invoke(&mut journey, action, &ids, &offer).unwrap();
    }
    for transition in [
        conduit_human::KeyTransition::Pressed,
        conduit_human::KeyTransition::Released,
    ] {
        assert!(journey.accept_play_input(key(0x04, transition)).unwrap());
    }
    let before = journey.projection();
    assert!(
        before
            .result
            .as_ref()
            .is_some_and(|result| !result.is_empty())
    );
    let mut door = crate::front_door::FrontDoor::new(
        before.host_id.clone(),
        before.boot_id.clone(),
        before.offer_generation,
        "profile",
        "build",
        "image",
        memory.source_document_id.clone(),
        memory.checked_plot_id.clone(),
        1,
        true,
    );
    door.observe_product(&journey).unwrap();
    invoke(&mut journey, JourneyAction::AdmitPlot, &ids, &offer).unwrap();
    let after = journey.projection();
    assert_eq!(after.body_id, before.body_id);
    assert_eq!(after.checked_plot_id, before.checked_plot_id);
    assert_eq!(after.result, before.result);
    assert_eq!(after.workload_revision, Some(1));
    assert!(after.active_play_id.is_none());
    let workspace = journey.workspace_projection().unwrap();
    assert_eq!(workspace.plots[0].plot, keyboard);
    assert_eq!(workspace.plots[0].title, NativePlot::KeyboardCanvas.title());
    assert!(!workspace.plots[0].foreground);
    assert_eq!(workspace.plots[1].plot, memory);
    assert_eq!(workspace.plots[1].title, NativePlot::MemoryLantern.title());
    assert!(workspace.plots[1].foreground);
    door.observe_product(&journey).unwrap();
    door.presentation().unwrap();
}
