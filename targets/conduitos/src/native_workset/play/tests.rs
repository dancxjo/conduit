use super::*;
use crate::native_workset::{self, NativePlot};
use alloc::{borrow::ToOwned, string::String};
use conduit_human::KeyModifiers;
use conduit_kernel::ValueStorage;

fn kernel() -> (PreparedNativeWorkset, NativeWorksetPlay) {
    let (ids, offer) = native_workset::tests::fixture();
    let wake = native_workset::tests::wake(&native_workset::inventory());
    let prepared = native_workset::prepare(&wake, &ids, &offer, "build").unwrap();
    let born =
        conduit_body::Body::born_with_plots(wake.workset.clone(), 1, "sign/born".into()).unwrap();
    let membership = conduit_body::BodyMembership::new(born.body_id.clone()).unwrap();
    let mut evidence =
        conduit_body::BodyBiographyEvidence::born(born.clone(), membership, "Test Body".into())
            .unwrap();
    let (body, exact_wake) = born.wake(2, "sign/woke".into()).unwrap();
    evidence.append_wake(body, exact_wake, 2).unwrap();
    let mut play = NativeWorksetPlay::prepare_with_biography(&prepared, &evidence).unwrap();
    let active = conduit_body::BodyPlayIdentity::bind(&prepared.plan, 3);
    play.start_for(&prepared.plan, &active).unwrap();
    (prepared, play)
}
fn key(usage: u8, transition: KeyTransition) -> KeyEvent {
    KeyEvent::new(usage, transition, KeyModifiers::NONE).unwrap()
}
fn type_key(play: &mut NativeWorksetPlay, plot: usize, usage: u8) -> String {
    play.input(plot, key(usage, KeyTransition::Pressed))
        .unwrap_or_else(|error| panic!("Plot {plot} usage {usage}: {error:?}"));
    let text = play.take_presentation(plot).unwrap().text().to_owned();
    play.input(plot, key(usage, KeyTransition::Released))
        .unwrap();
    text
}
fn index(prepared: &PreparedNativeWorkset, plot: NativePlot) -> usize {
    let identity = native_workset::resident(plot).unwrap();
    prepared
        .plan
        .plots
        .iter()
        .position(|plot| plot.plot == identity)
        .unwrap()
}

#[test]
fn tutorial_initial_view_and_event_cross_the_same_production_kernel() {
    let (prepared, mut play) = kernel();
    let tutorial = index(&prepared, NativePlot::Tour);
    let initial = play.take_application_view(tutorial).unwrap();
    let action = initial
        .actions
        .iter()
        .find(|action| action.id == "body.use-current")
        .unwrap();
    let event = conduit_presentation::ApplicationEvent {
        revision: initial.revision,
        action: action.id.clone(),
        kind: action.event,
        value: alloc::vec![],
    };
    play.application_event(tutorial, &event.encode(&initial).unwrap())
        .unwrap();
    assert_eq!(play.take_application_view(tutorial).unwrap(), initial);
    assert_eq!(
        play.take_application_request(tutorial),
        Some(native_workset::NativeApplicationRequest::Tutorial(
            native_workset::TutorialAction::UseCurrent
        ))
    );
}

#[test]
fn patchbay_inspects_the_selected_plot_and_emits_bounded_edit_authority() {
    let (prepared, mut play) = kernel();
    let tour = index(&prepared, NativePlot::Tour);
    let patchbay = index(&prepared, NativePlot::Patchbay);
    play.select_patchbay_target(patchbay, tour).unwrap();
    let initial = play.take_application_view(patchbay).unwrap();
    assert!(
        initial
            .nodes
            .iter()
            .any(|node| node.key == "plot" && node.text == "tour")
    );
    let inspect = conduit_presentation::ApplicationEvent {
        revision: initial.revision,
        action: patchbay_application::INSPECT_NEXT_ACTION_ID.into(),
        kind: conduit_presentation::ApplicationEventKind::Activate,
        value: alloc::vec::Vec::new(),
    };
    play.application_event(patchbay, &inspect.encode(&initial).unwrap())
        .unwrap();
    let inspected = play.take_application_view(patchbay).unwrap();
    assert_eq!(inspected.revision, initial.revision + 1);
    let edit = conduit_presentation::ApplicationEvent {
        revision: inspected.revision,
        action: patchbay_application::EDIT_CURRENT_ACTION_ID.into(),
        kind: conduit_presentation::ApplicationEventKind::Activate,
        value: alloc::vec::Vec::new(),
    };
    play.application_event(patchbay, &edit.encode(&inspected).unwrap())
        .unwrap();
    assert!(matches!(
        play.take_application_request(patchbay),
        Some(native_workset::NativeApplicationRequest::EditCurrent(
            patchbay_application::PatchbayApplicationRequest::EditCurrent { .. }
        ))
    ));
    let latest = play.take_application_view(patchbay).unwrap();
    let repeated_edit = conduit_presentation::ApplicationEvent {
        revision: latest.revision,
        action: patchbay_application::EDIT_CURRENT_ACTION_ID.into(),
        kind: conduit_presentation::ApplicationEventKind::Activate,
        value: alloc::vec::Vec::new(),
    };
    play.application_event(patchbay, &repeated_edit.encode(&latest).unwrap())
        .unwrap();
    play.cancel().unwrap();
    assert!(play.take_application_request(patchbay).is_none());
}
#[test]
fn switching_plots_keeps_independent_state_and_repeated_input_in_one_kernel() {
    let (prepared, mut play) = kernel();
    let canvas = index(&prepared, NativePlot::KeyboardCanvas);
    let memory = index(&prepared, NativePlot::MemoryLantern);
    assert_eq!(type_key(&mut play, canvas, 4), "A");
    assert_eq!(type_key(&mut play, memory, 5), "b");
    assert_eq!(type_key(&mut play, canvas, 6), "C");
    assert_eq!(type_key(&mut play, memory, 7), "bd");
    assert_eq!(type_key(&mut play, memory, 42), "b");
    assert_eq!(type_key(&mut play, memory, 42), "");
    assert_eq!(type_key(&mut play, canvas, 8), "E");
}
#[test]
fn release_keeps_press_owner_and_cancel_retires_every_pending_request() {
    let (prepared, mut play) = kernel();
    let canvas = index(&prepared, NativePlot::KeyboardCanvas);
    let memory = index(&prepared, NativePlot::MemoryLantern);
    let before = play.pending[memory].unwrap();
    play.input(canvas, key(4, KeyTransition::Pressed)).unwrap();
    play.take_presentation(canvas).unwrap();
    let held = play.pending[canvas].unwrap();
    assert!(play.owns_release(key(4, KeyTransition::Released)));
    assert!(!play.owns_release(key(5, KeyTransition::Released)));
    play.input(memory, key(4, KeyTransition::Released)).unwrap();
    assert!(!play.owns_release(key(4, KeyTransition::Released)));
    assert_eq!(play.pending[memory].unwrap().request, before.request);
    assert_ne!(play.pending[canvas].unwrap().request, held.request);
    play.cancel().unwrap();
    assert_eq!(play.scheduler.pending_host_call_count(), 0);
    assert_eq!(play.scheduler.values().used_items(), 0);
    assert_eq!(
        play.input(memory, key(5, KeyTransition::Pressed)),
        Err(PlayRefusal::Cancelled)
    );
    assert!(
        play.output(before, Some(&key(5, KeyTransition::Pressed).encode()))
            .is_err()
    );
    assert_eq!(play.scheduler.values().used_items(), 0);
}

#[test]
fn retained_editing_reaches_its_exact_bound_and_reports_capacity_failure() {
    let (prepared, mut play) = kernel();
    let memory = index(&prepared, NativePlot::MemoryLantern);
    for length in 1..=256 {
        assert_eq!(type_key(&mut play, memory, 4).len(), length);
    }
    assert_eq!(
        play.input(memory, key(4, KeyTransition::Pressed)),
        Err(PlayRefusal::HostFailure(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::StateCapacityExhausted,
            detail: 82
        }))
    );
    play.cancel().unwrap();
    assert_eq!(play.scheduler.values().used_items(), 0);
}

#[test]
fn thousands_of_interactions_reuse_fixed_storage_and_disclose_sign_eviction() {
    let (prepared, mut play) = kernel();
    let canvas = index(&prepared, NativePlot::KeyboardCanvas);
    let memory = index(&prepared, NativePlot::MemoryLantern);
    let items = play.scheduler.values().used_items();
    let bytes = play.scheduler.values().used_bytes();
    for _ in 0..512 {
        assert_eq!(type_key(&mut play, canvas, 4), "A");
        assert_eq!(type_key(&mut play, memory, 5), "b");
        assert_eq!(type_key(&mut play, memory, 42), "");
        assert_eq!(play.scheduler.values().used_items(), items);
        assert_eq!(play.scheduler.values().used_bytes(), bytes);
    }
    assert!(play.sign_retention_gap().is_some());
}

#[test]
fn unread_presentation_pressure_refuses_input_without_overwriting_it() {
    let (prepared, mut play) = kernel();
    let canvas = index(&prepared, NativePlot::KeyboardCanvas);
    play.input(canvas, key(4, KeyTransition::Pressed)).unwrap();
    let pending = play.pending[canvas].unwrap();
    assert_eq!(
        play.input(canvas, key(5, KeyTransition::Pressed)),
        Err(PlayRefusal::InputPressure)
    );
    assert_eq!(play.pending[canvas].unwrap().request, pending.request);
    assert_eq!(play.held[5], None);
    assert_eq!(play.take_presentation(canvas).unwrap().text(), "A");
    assert_eq!(type_key(&mut play, canvas, 5), "B");
}

#[test]
fn body_activation_rejects_wrong_plan_forged_play_replay_and_cancelled_restart() {
    let (ids, offer) = native_workset::tests::fixture();
    let wake = native_workset::tests::wake(&[NativePlot::KeyboardCanvas]);
    let prepared = native_workset::prepare(&wake, &ids, &offer, "build").unwrap();
    let mut kernel = NativeWorksetPlay::prepare(&prepared).unwrap();
    let active = conduit_body::BodyPlayIdentity::bind(&prepared.plan, 7);
    let mut forged = active.clone();
    forged.play_sequence += 1;
    assert_eq!(
        kernel.start_for(&prepared.plan, &forged),
        Err(PlayRefusal::Preparation)
    );
    assert!(kernel.pending_requests().iter().all(Option::is_none));
    assert!(kernel.active_body_play.is_none());
    let mut changed = prepared.plan.clone();
    changed.workload_revision += 1;
    assert_eq!(
        kernel.start_for(&changed, &active),
        Err(PlayRefusal::Preparation)
    );
    assert!(kernel.active_body_play.is_none());
    kernel.start_for(&prepared.plan, &active).unwrap();
    assert_eq!(kernel.active_body_play.as_ref(), Some(&active));
    assert_eq!(
        kernel.start_for(&prepared.plan, &active),
        Err(PlayRefusal::Preparation)
    );
    kernel.cancel().unwrap();
    assert!(kernel.active_body_play.is_none());
    assert_eq!(
        kernel.start_for(&prepared.plan, &active),
        Err(PlayRefusal::Preparation)
    );
}
