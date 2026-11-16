//! Deterministic renderer fixture: real owner/kernel effects are tested separately.
use super::*;
use crate::terminal_face_mask::TerminalKeyboard;
use conduit_face::*;
use conduit_human::InputButtonPhase;
#[allow(
    dead_code,
    clippy::duplicate_mod,
    reason = "reuse the checked Mask fixture with each proof selecting its needed entrance"
)]
#[path = "../fixture.rs"]
mod fixture;

const PATTERN: [ButtonSequenceStep<'static>; 2] = [
    ButtonSequenceStep {
        button_identity: "remote/confirm",
        pressed: true,
    },
    ButtonSequenceStep {
        button_identity: "remote/confirm",
        pressed: false,
    },
];
fn event(pressed: bool, sequence: u64) -> InputButtonTransition {
    InputButtonTransition::new(
        "remote/confirm".into(),
        if pressed {
            InputButtonPhase::Pressed
        } else {
            InputButtonPhase::Released
        },
        sequence,
    )
    .unwrap()
}
fn mask() -> TerminalFaceMask {
    let producer = fixture::producer_plan();
    let face = Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(producer.source_document_id),
            checked_plot_id: Some(producer.checked_plot_id),
            expanded_plot_id: Some(producer.expanded_plot_id),
            plan_id: Some(producer.plan_id),
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "task".into(),
            role: FaceRole::Document,
            name: "Task".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![FaceAction {
            identity: "complete".into(),
            intent: "task/complete".into(),
            target: "task".into(),
            name: "Complete".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap();
    TerminalFaceMask::prepare(face, 80, 24).unwrap()
}
fn bind(mask: &mut TerminalFaceMask) -> MaskShow {
    let prepared = fixture::prepared(mask.face());
    let receipt = mask.render(&mut Vec::new(), &prepared).unwrap();
    let available = prepared
        .transition(ManifestationLifecycle::Available, "fixture/flushed".into())
        .unwrap();
    mask.bind_show(receipt, available.clone()).unwrap();
    available
}
fn focused() -> (TerminalFaceMask, MaskShow) {
    let mut mask = mask();
    let show = bind(&mut mask);
    mask.input(TerminalInput::NextControl, &show, 0).unwrap();
    let show = bind(&mut mask);
    (mask, show)
}

#[test]
fn released_sequence_returns_the_same_offered_action_as_ordinary_enter() {
    let (mut mask, show) = focused();
    let mut sequence = TerminalButtonSequence::prepare(&mask, &show, &PATTERN).unwrap();
    let pending = sequence.observe(&mut mask, &event(true, 0), 1).unwrap();
    assert_eq!(
        pending.recognition,
        ButtonSequenceOutcome::Pending { matched: 1 }
    );
    assert!(pending.interaction.is_none());
    let completed = sequence
        .observe(&mut mask, &event(false, 1), 1)
        .unwrap()
        .interaction
        .unwrap();
    assert_eq!(
        sequence
            .observe(&mut mask, &event(false, 2), 2)
            .unwrap_err(),
        TerminalButtonSequenceRefusal::AlreadyRecognized
    );
    let (mut keyboard_mask, keyboard_show) = focused();
    let input = TerminalKeyboard::default().push(b'\r').unwrap().unwrap();
    let TerminalInputOutcome::Interaction(ordinary) =
        keyboard_mask.input(input, &keyboard_show, 1).unwrap()
    else {
        panic!("ordinary activation")
    };
    assert_eq!(completed.action_id, ordinary.action_id);
    assert_eq!(completed.target, ordinary.target);
    assert_eq!(completed.arguments, ordinary.arguments);
}

#[test]
fn changed_focus_show_and_explicit_loss_cannot_complete_an_old_sequence() {
    let (mut mask, show) = focused();
    let mut sequence = TerminalButtonSequence::prepare(&mask, &show, &PATTERN).unwrap();
    sequence.observe(&mut mask, &event(true, 0), 1).unwrap();
    mask.input(TerminalInput::Inspect, &show, 0).unwrap();
    assert!(sequence.observe(&mut mask, &event(false, 1), 1).is_err());
    assert!(sequence.observe(&mut mask, &event(true, 2), 1).is_err());
    let (mut fresh, current) = focused();
    let mut cancelled = TerminalButtonSequence::prepare(&fresh, &current, &PATTERN).unwrap();
    assert_eq!(cancelled.cancel(), ButtonSequenceOutcome::Cancelled);
    assert_eq!(
        cancelled
            .observe(&mut fresh, &event(true, 0), 1)
            .unwrap_err(),
        TerminalButtonSequenceRefusal::Cancelled
    );
}

#[test]
fn absent_focus_and_unacknowledged_or_forged_show_refuse_preparation() {
    let mut mask = mask();
    let show = fixture::prepared(mask.face());
    assert!(TerminalButtonSequence::prepare(&mask, &show, &PATTERN).is_err());
    let show = bind(&mut mask);
    assert!(TerminalButtonSequence::prepare(&mask, &show, &PATTERN).is_err());
    let mut read_only = TerminalFaceMask::prepare_read_only(mask.face().clone(), 80, 24).unwrap();
    let read_only_show = bind(&mut read_only);
    assert!(matches!(
        TerminalButtonSequence::prepare(&read_only, &read_only_show, &PATTERN),
        Err(TerminalButtonSequenceRefusal::Encounter(
            TerminalError::UnsupportedInput
        ))
    ));
    let (mask, mut show) = focused();
    show.show_id = serde_json::from_str("\"fixture/forged\"").unwrap();
    assert!(TerminalButtonSequence::prepare(&mask, &show, &PATTERN).is_err());
}

#[test]
fn broken_shortcut_source_preserves_the_ordinary_keyboard_alternative() {
    let (mut mask, show) = focused();
    let mut shortcut = TerminalButtonSequence::prepare(&mask, &show, &PATTERN).unwrap();
    shortcut.observe(&mut mask, &event(true, 0), 1).unwrap();
    let gap = shortcut.observe(&mut mask, &event(false, 2), 1).unwrap();
    assert_eq!(
        gap.recognition,
        ButtonSequenceOutcome::Rejected(ButtonSequenceRefusal::InputGap)
    );
    assert!(gap.interaction.is_none());
    assert!(shortcut
        .observe(&mut mask, &event(true, 3), 1)
        .unwrap()
        .interaction
        .is_none());
    let enter = TerminalKeyboard::default().push(b'\r').unwrap().unwrap();
    let TerminalInputOutcome::Interaction(ordinary) = mask.input(enter, &show, 1).unwrap() else {
        panic!("keyboard alternative must remain available")
    };
    assert_eq!(ordinary.action_id, "complete");
    assert!(shortcut.observe(&mut mask, &event(false, 4), 2).is_err());
}
