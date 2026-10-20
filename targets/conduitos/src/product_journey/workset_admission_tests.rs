use super::super::test_support::{fixture, invoke, key};
use super::*;
use conduit_birth_plot::BirthSelection;
use conduit_body::BodyWorkset;

#[test]
fn canonical_admission_before_foreground_preserves_identity_result_and_face() {
    let (ids, offer, mut journey) = fixture();
    let memory = native_workset::resident(NativePlot::MemoryLantern).unwrap();
    let keyboard = native_workset::resident(NativePlot::KeyboardCanvas).unwrap();
    let (existing, existing_kind, added, added_kind) = if keyboard < memory {
        (
            memory,
            NativePlot::MemoryLantern,
            keyboard,
            NativePlot::KeyboardCanvas,
        )
    } else {
        (
            keyboard,
            NativePlot::KeyboardCanvas,
            memory,
            NativePlot::MemoryLantern,
        )
    };
    assert!(
        added < existing,
        "regression needs insertion before the existing resident"
    );
    journey
        .birth_from_creche(BirthSelection {
            revision: 1,
            friendly_name: "Ordered Body".into(),
            workset: BodyWorkset::one(existing.clone()).unwrap(),
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
        existing.source_document_id.clone(),
        existing.checked_plot_id.clone(),
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
    assert_eq!(workspace.plots[0].plot, added);
    assert_eq!(workspace.plots[0].title, added_kind.title());
    assert!(!workspace.plots[0].foreground);
    assert_eq!(workspace.plots[1].plot, existing);
    assert_eq!(workspace.plots[1].title, existing_kind.title());
    assert!(workspace.plots[1].foreground);
    door.observe_product(&journey).unwrap();
    door.face().unwrap();
}

#[test]
fn creche_eligibility_cannot_bypass_current_workset_admission() {
    let (ids, mut offer, mut journey) = fixture();
    let plot = NativePlot::KeyboardCanvas;
    assert_eq!(native_workset::eligibility(plot, &offer), Ok(()));
    journey
        .birth_from_creche(BirthSelection {
            revision: 1,
            friendly_name: "Deferred admission".into(),
            workset: BodyWorkset::one(native_workset::resident(plot).unwrap()).unwrap(),
        })
        .unwrap();
    assert_eq!(journey.status(), JourneyStatus::BornLulled);
    assert!(journey.current_plan().is_none());
    assert!(journey.current_play().is_none());
    let before = journey.biography().unwrap().clone();
    // Selection evidence from the earlier offer grants no execution authority.
    offer.keyboard = None;
    assert_eq!(
        invoke(&mut journey, JourneyAction::Wake, &ids, &offer),
        Err(JourneyError::Workset(native_workset::WorksetRefusal::Host))
    );
    assert_eq!(journey.biography(), Some(&before));
    assert!(journey.current_plan().is_none());
    assert!(journey.current_play().is_none());
    assert!(journey.kernel.is_none());
}
