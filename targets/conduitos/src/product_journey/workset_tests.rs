use super::test_support::{fixture, invoke, key};
use super::*;
use crate::machine::{
    BaseError, IdleBase, InterruptBase, InterruptState, MonotonicClockBase, SerialBase,
};
use alloc::vec::Vec;
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
fn native_birth_keeps_four_forms_in_one_body_plan_play_and_switches_only_foreground() {
    let (ids, offer, mut journey) = born();
    let born = journey.workspace_projection().unwrap();
    assert_eq!(born.forms.len(), 4);
    assert!(born.forms.iter().all(|form| form.input.is_none()));
    run(&mut journey, &ids, &offer);
    let started = journey.projection();
    assert_eq!(started.status, JourneyStatus::QuiescentAwaitingInput);
    assert_eq!(started.gear_ids.len(), 14);
    assert_eq!(started.cord_ids.len(), 10);
    let admitted = journey.workspace_projection().unwrap();
    assert!(admitted.forms.iter().all(|form| {
        form.input.as_ref().is_some_and(|input| {
            input.form == form.form
                && matches!(
                    input.value_kind.as_str(),
                    conduit_human::KEY_EVENT_INFO_ID
                        | conduit_presentation::APPLICATION_EVENT_INFO_ID
                )
        })
    }));
    select(&mut journey, NativeForm::KeyboardCanvas);
    assert_eq!(
        journey.foreground_input_owner().unwrap().form,
        native_workset::resident(NativeForm::KeyboardCanvas).unwrap()
    );
    type_key(&mut journey, 4);
    assert_eq!(journey.projection().result.as_deref(), Some("A"));
    select(&mut journey, NativeForm::MemoryLantern);
    assert_eq!(
        journey.foreground_input_owner().unwrap().form,
        native_workset::resident(NativeForm::MemoryLantern).unwrap()
    );
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
    assert_eq!(final_view.status, JourneyStatus::QuiescentAwaitingInput);
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
fn returning_to_resident_tour_preserves_state_and_body_execution_identity() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    let execution = journey.projection();
    select(&mut journey, NativeForm::Tour);
    let initial = journey.foreground_application_view().unwrap().clone();
    assert!(
        journey
            .accept_application_event(&conduit_presentation::ApplicationEvent {
                revision: initial.revision,
                action: conduit_tour_model::OPEN_PATCHBAY_ACTION_ID.into(),
                kind: conduit_presentation::ApplicationEventKind::Activate,
                value: alloc::vec::Vec::new(),
            })
            .unwrap()
    );
    let tour_revision = initial.revision + 1;
    assert!(
        journey
            .foreground_application_view()
            .unwrap()
            .nodes
            .iter()
            .any(|node| node.key == "form" && node.text == "tour")
    );
    select(&mut journey, NativeForm::MemoryLantern);
    type_key(&mut journey, 5);
    select(&mut journey, NativeForm::Tour);
    assert_eq!(
        journey.foreground_application_view().unwrap().revision,
        tour_revision
    );
    let returned = journey.projection();
    assert_eq!(returned.body_id, execution.body_id);
    assert_eq!(returned.plan_id, execution.plan_id);
    assert_eq!(returned.active_play_id, execution.active_play_id);
}

#[test]
fn resident_patchbay_opens_the_previously_used_exact_form_and_plan() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    select(&mut journey, NativeForm::Tour);
    let before = journey.projection();
    let expected_form = before.expanded_form_id.clone().unwrap();
    let expected_plan = before.plan_id.clone().unwrap();
    select(&mut journey, NativeForm::Patchbay);
    let view = journey.foreground_application_view().unwrap();
    assert!(
        view.nodes.iter().any(|node| {
            node.key == "identity"
                && node.text.contains(expected_form.as_str())
                && node.text.contains(expected_plan.as_str())
        }),
        "{view:?} expected {expected_form:?} {expected_plan:?}"
    );
    let execution = journey.projection();
    assert_eq!(execution.body_id, before.body_id);
    select(&mut journey, NativeForm::MemoryLantern);
    select(&mut journey, NativeForm::Patchbay);
    assert!(
        journey
            .foreground_application_view()
            .unwrap()
            .nodes
            .iter()
            .any(|node| node.key == "form" && node.text == "memory_lantern")
    );
    assert_eq!(
        journey.projection().active_play_id,
        execution.active_play_id
    );
}

#[test]
fn resident_tour_run_crosses_the_real_plan_play_and_returns_proof_to_the_same_body() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    select(&mut journey, NativeForm::Tour);
    let execution = journey.projection();
    let view = journey.foreground_application_view().unwrap().clone();
    journey
        .accept_application_event(&conduit_presentation::ApplicationEvent {
            revision: view.revision,
            action: conduit_tour_model::RUN_ACTION_ID.into(),
            kind: conduit_presentation::ApplicationEventKind::Activate,
            value: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        journey.take_application_request(),
        Some(native_workset::NativeApplicationRequest::RunTour)
    );
    let mut prepared = crate::tour_play::prepare(&ids, &offer, "build").unwrap();
    let mut clock = TestClock::default();
    let mut serial = TestSerial::default();
    let mut interrupts = TestInterrupts::default();
    let mut idle = TestIdle::default();
    let evidence = crate::tour_play::run(
        &mut prepared,
        &mut clock,
        &mut serial,
        &mut interrupts,
        &mut idle,
    )
    .unwrap();
    journey.complete_tour_run(&evidence).unwrap();
    assert_eq!(serial.0, [conduit_tour_model::CANONICAL_RESULT.as_bytes()]);
    assert!(
        journey
            .foreground_application_view()
            .unwrap()
            .nodes
            .iter()
            .any(|node| { node.key == "result" && node.text.contains("Result visible") })
    );
    let after = journey.projection();
    assert_eq!(after.body_id, execution.body_id);
    assert_eq!(after.plan_id, execution.plan_id);
    assert_eq!(after.active_play_id, execution.active_play_id);
}

#[derive(Default)]
struct TestClock(u64);
impl MonotonicClockBase for TestClock {
    fn now(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}
#[derive(Default)]
struct TestSerial(Vec<Vec<u8>>);
impl SerialBase for TestSerial {
    fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
        self.0.push(bytes.into());
        Ok(())
    }
    fn presentation_count(&self) -> u32 {
        self.0.len() as u32
    }
}
#[derive(Default)]
struct TestInterrupts(bool);
impl InterruptBase for TestInterrupts {
    fn enable(&mut self) {
        self.0 = true;
    }
    fn disable(&mut self) -> InterruptState {
        let state = InterruptState { enabled: self.0 };
        self.0 = false;
        state
    }
    fn restore(&mut self, state: InterruptState) {
        self.0 = state.enabled;
    }
    fn is_enabled(&self) -> bool {
        self.0
    }
}
#[derive(Default)]
struct TestIdle(u32);
impl IdleBase for TestIdle {
    fn wait_for_interrupt(&mut self) -> Result<(), BaseError> {
        self.0 += 1;
        Ok(())
    }
    fn idle_count(&self) -> u32 {
        self.0
    }
}

#[test]
fn lull_retains_all_forms_and_next_wake_prepares_fresh_plan_play() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    let first = journey.projection();
    type_key(&mut journey, 4);
    invoke(&mut journey, JourneyAction::Stop, &ids, &offer).unwrap();
    invoke(&mut journey, JourneyAction::Lull, &ids, &offer).unwrap();
    assert_eq!(journey.workspace_projection().unwrap().forms.len(), 4);
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
