use super::test_support::{fixture, invoke, key};
use super::*;
use alloc::vec::Vec;
use conduit_birth_plot::BirthSelection;
use conduit_body::BodyWorkset;
use conduit_human::KeyTransition;

fn born() -> (BootIdentities, HostOffer<'static>, ProductJourney) {
    let (ids, offer, mut journey) = fixture();
    journey
        .birth_from_creche(BirthSelection {
            revision: 3,
            friendly_name: "Roseau".into(),
            workset: BodyWorkset::from_plots(
                native_workset::inventory()
                    .into_iter()
                    .map(|plot| native_workset::resident(plot).unwrap()),
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
fn select(journey: &mut ProductJourney, plot: NativePlot) {
    journey
        .select_plot(&native_workset::resident(plot).unwrap(), journey.revision())
        .unwrap();
}

fn request_mask_change(
    journey: &mut ProductJourney,
) -> patchbay_application::PatchbayApplicationRequest {
    let view = journey.foreground_application_view().unwrap().clone();
    let action = view
        .actions
        .iter()
        .find(|action| action.id == patchbay_application::CHANGE_MASKS_ACTION_ID)
        .unwrap();
    assert!(
        journey
            .accept_application_event(&conduit_presentation::ApplicationEvent {
                revision: view.revision,
                action: action.id.clone(),
                kind: action.event,
                value: Vec::new(),
            })
            .unwrap()
    );
    match journey.take_application_request().unwrap() {
        native_workset::NativeApplicationRequest::EditCurrent(request) => request,
        _ => panic!("Patchbay Mask action must cross the typed application seam"),
    }
}

#[test]
fn native_birth_keeps_four_plots_in_one_body_plan_play_and_switches_only_foreground() {
    let (ids, offer, mut journey) = born();
    let born = journey.workspace_projection().unwrap();
    assert_eq!(born.plots.len(), 4);
    assert!(born.plots.iter().all(|plot| plot.input.is_none()));
    run(&mut journey, &ids, &offer);
    let started = journey.projection();
    assert_eq!(started.status, JourneyStatus::QuiescentAwaitingInput);
    assert_eq!(started.gear_ids.len(), 14);
    assert_eq!(started.cord_ids.len(), 10);
    let mask = started.mask.as_ref().unwrap();
    assert_eq!(mask.route_disposition, "planned-route-awaiting-show");
    assert!(mask.shows.is_empty());
    assert!(mask.show_id.is_none());
    let admitted = journey.workspace_projection().unwrap();
    assert!(admitted.plots.iter().all(|plot| {
        plot.input.as_ref().is_some_and(|input| {
            input.plot == plot.plot
                && matches!(
                    input.value_kind.as_str(),
                    conduit_human::KEY_EVENT_INFO_ID
                        | conduit_presentation::APPLICATION_EVENT_INFO_ID
                )
        })
    }));
    select(&mut journey, NativePlot::KeyboardCanvas);
    assert_eq!(
        journey.foreground_input_owner().unwrap().plot,
        native_workset::resident(NativePlot::KeyboardCanvas).unwrap()
    );
    type_key(&mut journey, 4);
    assert_eq!(journey.projection().result.as_deref(), Some("A"));
    select(&mut journey, NativePlot::MemoryLantern);
    assert_eq!(
        journey.foreground_input_owner().unwrap().plot,
        native_workset::resident(NativePlot::MemoryLantern).unwrap()
    );
    type_key(&mut journey, 5);
    type_key(&mut journey, 6);
    assert_eq!(journey.projection().result.as_deref(), Some("bc"));
    select(&mut journey, NativePlot::KeyboardCanvas);
    type_key(&mut journey, 7);
    assert_eq!(journey.projection().result.as_deref(), Some("AD"));
    select(&mut journey, NativePlot::MemoryLantern);
    type_key(&mut journey, 42);
    assert_eq!(journey.projection().result.as_deref(), Some("b"));
    let final_view = journey.projection();
    assert_eq!(final_view.status, JourneyStatus::QuiescentAwaitingInput);
    assert_eq!(final_view.body_id, started.body_id);
    assert_eq!(final_view.plan_id, started.plan_id);
    assert_eq!(final_view.active_play_id, started.active_play_id);
    assert_eq!(final_view.input_count, 10);
    let current = journey.workspace_projection().unwrap();
    assert_eq!(
        current.plots.iter().filter(|plot| plot.foreground).count(),
        1
    );
    let before = journey.projection();
    assert_eq!(
        journey.select_next_plot(0),
        Err(JourneyError::StalePresentation)
    );
    assert_eq!(journey.projection(), before);
}

#[test]
fn resident_patchbay_replans_its_own_graphical_and_speech_masks() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    select(&mut journey, NativePlot::Patchbay);
    let initial = journey.projection();
    let initial_view = journey.foreground_application_view().unwrap();
    assert_eq!(
        initial_view
            .nodes
            .iter()
            .filter(|node| node.key.starts_with("mask-stage-"))
            .count(),
        1
    );
    let initial_graphics = initial_view
        .nodes
        .iter()
        .find(|node| node.key == "mask-facts-0")
        .unwrap()
        .value
        .clone();

    let add = request_mask_change(&mut journey);
    let stale_add = add.clone();
    journey.replan_masks(add, &ids, &offer, "build").unwrap();
    let parallel = journey.projection();
    assert_eq!(parallel.body_id, initial.body_id);
    assert_ne!(parallel.plan_id, initial.plan_id);
    assert_ne!(parallel.active_play_id, initial.active_play_id);
    assert_eq!(
        journey
            .foreground_application_view()
            .unwrap()
            .nodes
            .iter()
            .filter(|node| node.key.starts_with("mask-stage-"))
            .count(),
        2
    );
    let before_stale = journey.projection();
    assert_eq!(
        journey.replan_masks(stale_add, &ids, &offer, "build"),
        Err(JourneyError::StalePresentation)
    );
    assert_eq!(journey.projection(), before_stale);

    let remove_graphics = request_mask_change(&mut journey);
    journey
        .replan_masks(remove_graphics, &ids, &offer, "build")
        .unwrap();
    let speech = journey.projection();
    assert_eq!(speech.body_id, initial.body_id);
    assert_ne!(speech.plan_id, parallel.plan_id);
    let mask = speech
        .mask
        .as_ref()
        .expect("replacement retains Mask truth");
    assert!(mask.actions.contains(&"doff"));
    assert_eq!(mask.route_disposition, "planned-route-awaiting-show");
    assert_eq!(mask.planning_disposition, "not-required");
    assert!(mask.show_id.is_none() && mask.manifestation_id.is_none());
    assert!(mask.presentation_id.is_none() && mask.presentation_revision.is_none());
    assert!(mask.mask_actions.is_empty() && mask.shows.is_empty());
    assert_eq!(mask.kernel_signs, 0);
    assert_eq!(mask.fore_endpoints, 0);
    assert!(
        mask.mask_plan_ids
            .iter()
            .all(|plan| Some(plan) != speech.plan_id.as_ref()),
        "application and Mask Plan identities remain distinct"
    );
    let speech_view = journey.foreground_application_view().unwrap();
    assert_eq!(
        speech_view
            .nodes
            .iter()
            .filter(|node| node.key.starts_with("mask-stage-"))
            .count(),
        1
    );
    assert!(
        speech_view
            .nodes
            .iter()
            .any(|node| { node.key == "mask-impl-0" && node.value.contains("test-speech") })
    );

    let restore_graphics = request_mask_change(&mut journey);
    journey
        .replan_masks(restore_graphics, &ids, &offer, "build")
        .unwrap();
    let restored = journey.projection();
    assert_eq!(restored.body_id, initial.body_id);
    assert_ne!(restored.plan_id, speech.plan_id);
    let restored_view = journey.foreground_application_view().unwrap();
    assert_eq!(
        restored_view
            .nodes
            .iter()
            .filter(|node| node.key.starts_with("mask-stage-"))
            .count(),
        2
    );
    assert_eq!(
        restored_view
            .nodes
            .iter()
            .find(|node| node.key == "mask-facts-0")
            .unwrap()
            .value,
        initial_graphics
    );
    assert!(
        restored_view
            .nodes
            .iter()
            .any(|node| { node.key == "mask-stage-0" && node.text.contains("unavailable") })
    );
}

#[test]
fn returning_to_resident_tutorial_preserves_biography_and_body_execution_identity() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    let execution = journey.projection();
    select(&mut journey, NativePlot::Tour);
    let initial = journey.foreground_application_view().unwrap().clone();
    select(&mut journey, NativePlot::MemoryLantern);
    type_key(&mut journey, 5);
    select(&mut journey, NativePlot::Tour);
    assert_eq!(journey.foreground_application_view().unwrap(), &initial);
    assert_eq!(journey.projection().body_id, execution.body_id);
    assert_eq!(
        journey.projection().active_play_id,
        execution.active_play_id
    );
}

#[test]
fn resident_patchbay_opens_the_previously_used_exact_plot_and_plan() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    select(&mut journey, NativePlot::Tour);
    let before = journey.projection();
    let expected_plot = before.expanded_plot_id.clone().unwrap();
    let expected_plan = before.plan_id.clone().unwrap();
    select(&mut journey, NativePlot::Patchbay);
    let view = journey.foreground_application_view().unwrap();
    assert!(
        view.nodes.iter().any(|node| {
            node.key == "identity"
                && node.text.contains(expected_plot.as_str())
                && node.text.contains(expected_plan.as_str())
        }),
        "{view:?} expected {expected_plot:?} {expected_plan:?}"
    );
    let execution = journey.projection();
    assert_eq!(execution.body_id, before.body_id);
    select(&mut journey, NativePlot::MemoryLantern);
    select(&mut journey, NativePlot::Patchbay);
    assert!(
        journey
            .foreground_application_view()
            .unwrap()
            .nodes
            .iter()
            .any(|node| node.key == "plot" && node.text == "memory_lantern")
    );
    assert_eq!(
        journey.projection().active_play_id,
        execution.active_play_id
    );
}

#[test]
fn current_tutorial_refuses_retired_chapter_actions_without_changing_play() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    select(&mut journey, NativePlot::Tour);
    let before = journey.projection();
    let view = journey.foreground_application_view().unwrap().clone();
    assert_eq!(
        journey.accept_application_event(&conduit_presentation::ApplicationEvent {
            revision: view.revision,
            action: conduit_tour_model::RUN_ACTION_ID.into(),
            kind: conduit_presentation::ApplicationEventKind::Activate,
            value: Vec::new(),
        }),
        Err(JourneyError::WrongTarget)
    );
    assert_eq!(journey.projection(), before);
}

#[test]
fn current_tutorial_request_comes_from_shared_biography_guidance() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    select(&mut journey, NativePlot::Tour);
    let view = journey.foreground_application_view().unwrap().clone();
    let action = view
        .actions
        .iter()
        .find(|action| action.id == "body.use-current")
        .unwrap();
    journey
        .accept_application_event(&conduit_presentation::ApplicationEvent {
            revision: view.revision,
            action: action.id.clone(),
            kind: action.event,
            value: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        journey.take_application_request(),
        Some(native_workset::NativeApplicationRequest::Tutorial(
            native_workset::TutorialAction::UseCurrent
        ))
    );
}

#[test]
fn lull_retains_all_plots_and_next_wake_prepares_fresh_plan_play() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    let first = journey.projection();
    type_key(&mut journey, 4);
    invoke(&mut journey, JourneyAction::Stop, &ids, &offer).unwrap();
    invoke(&mut journey, JourneyAction::Lull, &ids, &offer).unwrap();
    assert_eq!(journey.workspace_projection().unwrap().plots.len(), 4);
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
    assert!(journey.foreground_input_owner().is_none());
    assert!(
        !journey
            .accept_play_input(key(4, KeyTransition::Released))
            .unwrap()
    );
    assert_eq!(journey.workspace_projection().unwrap().plots, before.plots);
    assert_eq!(
        journey.workspace_projection().unwrap().body_id,
        before.body_id
    );
}

#[test]
fn foreground_selection_before_admission_does_not_invent_a_plan_or_play() {
    let (ids, offer, mut journey) = born();
    let before = journey.projection();
    select(&mut journey, NativePlot::MemoryLantern);
    let selected = journey.projection();
    assert_eq!(selected.status, JourneyStatus::BornLulled);
    assert_eq!(selected.body_id, before.body_id);
    assert!(selected.plan_id.is_none() && selected.active_play_id.is_none());
    invoke(&mut journey, JourneyAction::Wake, &ids, &offer).unwrap();
    select(&mut journey, NativePlot::KeyboardCanvas);
    let waiting = journey.projection();
    assert_eq!(waiting.status, JourneyStatus::Awake);
    assert!(waiting.plan_id.is_some() && waiting.active_play_id.is_none());
}
