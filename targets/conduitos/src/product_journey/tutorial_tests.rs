use super::test_support::{fixture, invoke};
use super::*;
use conduit_body::BodyWorkset;
use conduit_presentation::{ApplicationEvent, ApplicationEventKind};

#[test]
fn native_tutorial_uses_same_body_biography_through_wake_and_lull() {
    let (ids, offer, mut journey) = fixture();
    journey
        .birth_from_creche(conduit_birth_plot::BirthSelection {
            revision: 1,
            friendly_name: "Shared Workspace".into(),
            workset: BodyWorkset::one(native_workset::resident(NativePlot::Tour).unwrap()).unwrap(),
        })
        .unwrap();
    let born = journey.biography().unwrap().body_id.clone();
    let initial = journey.tutorial_view().unwrap();
    assert!(initial.actions.iter().any(|a| a.id == "body.wake"));
    journey
        .accept_tutorial_action(
            &ApplicationEvent {
                revision: initial.revision,
                action: "body.wake".into(),
                kind: ApplicationEventKind::Activate,
                value: alloc::vec![],
            },
            &ids,
            &offer,
            "build",
        )
        .unwrap();
    let evidence = journey.biography().unwrap();
    assert_eq!(evidence.body_id, born);
    evidence.validate().unwrap();
    assert!(
        evidence.wakes[0]
            .events
            .iter()
            .any(|e| matches!(e, WakeLifecycleEvent::PlayStarted { .. }))
    );
    let expected = conduit_tutorial_plot::presentation_from_evidence(
        evidence,
        evidence.records.len() as u32,
        conduit_tutorial_plot::TutorialPlayback::Playing,
    )
    .unwrap()
    .lower()
    .unwrap();
    assert_eq!(journey.tutorial_view().unwrap(), expected);
    assert_eq!(journey.foreground_application_view().unwrap(), &expected);
    assert!(!expected.actions.iter().any(|a| a.id.starts_with("tour.")));
    invoke(&mut journey, JourneyAction::Lull, &ids, &offer).unwrap();
    assert_eq!(journey.biography().unwrap().body_id, born);
    assert!(
        journey
            .tutorial_view()
            .unwrap()
            .actions
            .iter()
            .any(|a| a.id == "body.wake")
    );
    assert_eq!(
        journey.apply_tutorial_action(
            native_workset::TutorialAction::OpenLibrary,
            &ids,
            &offer,
            "build"
        ),
        Err(TutorialRefusal::LibraryUnavailable)
    );
    assert_eq!(
        journey.apply_tutorial_action(
            native_workset::TutorialAction::InviteHost,
            &ids,
            &offer,
            "build"
        ),
        Err(TutorialRefusal::InvitationUnavailable)
    );
}

#[test]
fn finite_body_history_reserves_lull_and_refuses_next_wake_without_storage() {
    let (ids, offer, mut journey) = fixture();
    journey
        .birth_from_creche(conduit_birth_plot::BirthSelection {
            revision: 1,
            friendly_name: "Bounded Workspace".into(),
            workset: BodyWorkset::one(native_workset::resident(NativePlot::Tour).unwrap()).unwrap(),
        })
        .unwrap();
    for _ in 0..128 {
        let before = journey.projection();
        let evidence = journey.biography().unwrap().clone();
        match invoke(&mut journey, JourneyAction::Wake, &ids, &offer) {
            Err(JourneyError::Lifecycle(BodyLifecycleSessionError::ArchivePersistenceRequired)) => {
                assert_eq!(journey.projection(), before);
                assert_eq!(journey.biography(), Some(&evidence));
                assert!(journey.kernel.is_none());
                assert_eq!(journey.body().unwrap().state, BodyState::Lulled);
                return;
            }
            Ok(()) => {}
            Err(error) => panic!("unexpected Wake refusal: {error:?}"),
        }
        for action in [
            JourneyAction::Plan,
            JourneyAction::Play,
            JourneyAction::Lull,
        ] {
            invoke(&mut journey, action, &ids, &offer).unwrap();
        }
    }
    panic!("native history requires a finite archive persistence boundary");
}

#[test]
fn tutorial_use_current_selects_real_resident_input_without_rebirth_or_replay() {
    let (ids, offer, mut journey) = fixture();
    let memory = native_workset::resident(NativePlot::MemoryLantern).unwrap();
    let tutorial = native_workset::resident(NativePlot::Tour).unwrap();
    journey
        .birth_from_creche(conduit_birth_plot::BirthSelection {
            revision: 1,
            friendly_name: "Working Body".into(),
            workset: BodyWorkset::from_plots([memory.clone(), tutorial.clone()]).unwrap(),
        })
        .unwrap();
    for action in [
        JourneyAction::Wake,
        JourneyAction::Plan,
        JourneyAction::Play,
    ] {
        invoke(&mut journey, action, &ids, &offer).unwrap();
    }
    journey.select_plot(&memory, journey.revision()).unwrap();
    journey.select_plot(&tutorial, journey.revision()).unwrap();
    let before = journey.projection();
    let view = journey.tutorial_view().unwrap();
    journey
        .accept_tutorial_action(
            &ApplicationEvent {
                revision: view.revision,
                action: "body.use-current".into(),
                kind: ApplicationEventKind::Activate,
                value: alloc::vec![],
            },
            &ids,
            &offer,
            "build",
        )
        .unwrap();
    assert_eq!(journey.projection().body_id, before.body_id);
    assert_eq!(journey.projection().active_play_id, before.active_play_id);
    assert_eq!(
        journey.projection().checked_plot_id,
        Some(memory.checked_plot_id)
    );
    journey
        .accept_play_input(super::test_support::key(
            4,
            conduit_human::KeyTransition::Pressed,
        ))
        .unwrap();
    assert_eq!(journey.foreground_result(), Some("a"));
}
