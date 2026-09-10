use super::*;
use crate::native_workset::{self, NativeForm};
use alloc::{borrow::ToOwned, string::String};
use conduit_human::KeyModifiers;
use conduit_kernel::ValueStorage;

fn kernel() -> (PreparedNativeWorkset, NativeWorksetPlay) {
    let (ids, offer) = native_workset::tests::fixture();
    let wake = native_workset::tests::wake(&native_workset::inventory());
    let prepared = native_workset::prepare(&wake, &ids, &offer, "build").unwrap();
    let mut play = NativeWorksetPlay::prepare(&prepared).unwrap();
    play.start().unwrap();
    (prepared, play)
}
fn key(usage: u8, transition: KeyTransition) -> KeyEvent {
    KeyEvent::new(usage, transition, KeyModifiers::NONE).unwrap()
}
fn type_key(play: &mut NativeWorksetPlay, form: usize, usage: u8) -> String {
    play.input(form, key(usage, KeyTransition::Pressed))
        .unwrap_or_else(|error| panic!("Form {form} usage {usage}: {error:?}"));
    let text = play.take_presentation(form).unwrap().text().to_owned();
    play.input(form, key(usage, KeyTransition::Released))
        .unwrap();
    text
}
fn index(prepared: &PreparedNativeWorkset, form: NativeForm) -> usize {
    let identity = native_workset::resident(form).unwrap();
    prepared
        .plan
        .forms
        .iter()
        .position(|form| form.form == identity)
        .unwrap()
}
#[test]
fn switching_forms_keeps_independent_state_and_repeated_input_in_one_kernel() {
    let (prepared, mut play) = kernel();
    let canvas = index(&prepared, NativeForm::KeyboardCanvas);
    let memory = index(&prepared, NativeForm::MemoryLantern);
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
    let canvas = index(&prepared, NativeForm::KeyboardCanvas);
    let memory = index(&prepared, NativeForm::MemoryLantern);
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
    assert_eq!(play.scheduler.pending_host_operation_count(), 0);
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
    let memory = index(&prepared, NativeForm::MemoryLantern);
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
    let canvas = index(&prepared, NativeForm::KeyboardCanvas);
    let memory = index(&prepared, NativeForm::MemoryLantern);
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
    let canvas = index(&prepared, NativeForm::KeyboardCanvas);
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
