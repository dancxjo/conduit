use conduit_human::{
    ButtonSequence, ButtonSequenceOutcome as Outcome, ButtonSequenceRefusal as Refusal,
    ButtonSequenceStep as Step, InputButtonPhase, InputButtonTransition,
};

fn event(button: &str, pressed: bool, sequence: u64) -> InputButtonTransition {
    InputButtonTransition::new(
        button.into(),
        if pressed {
            InputButtonPhase::Pressed
        } else {
            InputButtonPhase::Released
        },
        sequence,
    )
    .unwrap()
}
const DOUBLE: [Step<'static>; 4] = [
    Step {
        button_identity: "remote/confirm",
        pressed: true,
    },
    Step {
        button_identity: "remote/confirm",
        pressed: false,
    },
    Step {
        button_identity: "remote/confirm",
        pressed: true,
    },
    Step {
        button_identity: "remote/confirm",
        pressed: false,
    },
];

#[test]
fn completed_released_sequence_recognizes_once_and_held_reports_do_not_advance() {
    let mut recognition = ButtonSequence::prepare(&DOUBLE).unwrap();
    for (sequence, pressed, matched) in
        [(10, true, 1), (11, true, 1), (12, false, 2), (13, true, 3)]
    {
        assert_eq!(
            recognition.observe(&event("remote/confirm", pressed, sequence)),
            Outcome::Pending { matched }
        );
    }
    assert_eq!(
        recognition.observe(&event("remote/confirm", false, 14)),
        Outcome::Recognized
    );
    assert_eq!(
        recognition.cancel(),
        Outcome::Rejected(Refusal::AlreadyRecognized)
    );
    assert_eq!(
        recognition.observe(&event("remote/confirm", true, 15)),
        Outcome::Rejected(Refusal::AlreadyRecognized)
    );
}

#[test]
fn cancelling_held_pattern_drains_release_before_fresh_recognition() {
    let mut recognition = ButtonSequence::prepare(&DOUBLE).unwrap();
    recognition.observe(&event("remote/confirm", true, 0));
    assert_eq!(recognition.cancel(), Outcome::Cancelled);
    assert_eq!(
        recognition.observe(&event("remote/confirm", true, 1)),
        Outcome::Pending { matched: 0 }
    );
    assert_eq!(
        recognition.observe(&event("remote/confirm", false, 2)),
        Outcome::Pending { matched: 0 }
    );
    for (sequence, pressed) in [(3, true), (4, false), (5, true)] {
        assert!(matches!(
            recognition.observe(&event("remote/confirm", pressed, sequence)),
            Outcome::Pending { .. }
        ));
    }
    assert_eq!(
        recognition.observe(&event("remote/confirm", false, 6)),
        Outcome::Recognized
    );
}

#[test]
fn ordered_occurrence_gap_and_replay_cancel_partial_recognition() {
    let mut recognition = ButtonSequence::prepare(&DOUBLE).unwrap();
    recognition.observe(&event("remote/confirm", true, 20));
    assert_eq!(
        recognition.observe(&event("remote/confirm", false, 22)),
        Outcome::Rejected(Refusal::InputGap)
    );
    assert_eq!(
        recognition.observe(&event("remote/confirm", true, 21)),
        Outcome::Rejected(Refusal::OutOfOrder)
    );
    assert_eq!(
        recognition.observe(&event("remote/confirm", false, 23)),
        Outcome::Pending { matched: 0 }
    );
    assert_eq!(
        recognition.observe(&event("remote/confirm", false, 24)),
        Outcome::Rejected(Refusal::UnmatchedRelease)
    );
}

#[test]
fn unmatched_foreign_and_wrong_order_buttons_cannot_complete_a_pattern() {
    let pattern = [
        DOUBLE[0],
        DOUBLE[1],
        Step {
            button_identity: "remote/back",
            pressed: true,
        },
        Step {
            button_identity: "remote/back",
            pressed: false,
        },
    ];
    let mut recognition = ButtonSequence::prepare(&pattern).unwrap();
    assert_eq!(
        recognition.observe(&event("remote/foreign", true, 0)),
        Outcome::Rejected(Refusal::UnexpectedButton)
    );
    assert_eq!(
        recognition.observe(&event("remote/back", true, 1)),
        Outcome::Rejected(Refusal::Mismatch)
    );
    assert_eq!(
        recognition.observe(&event("remote/back", false, 2)),
        Outcome::Pending { matched: 0 }
    );
    assert_eq!(
        recognition.observe(&event("remote/confirm", true, 3)),
        Outcome::Pending { matched: 1 }
    );
}

#[test]
fn preparation_refuses_empty_oversized_and_unbalanced_patterns() {
    assert!(matches!(ButtonSequence::prepare(&[]), Err(Refusal::Empty)));
    assert!(matches!(
        ButtonSequence::prepare(&[DOUBLE[0]; 17]),
        Err(Refusal::Capacity)
    ));
    assert!(matches!(
        ButtonSequence::prepare(&DOUBLE[..1]),
        Err(Refusal::UnbalancedPattern)
    ));
    assert!(matches!(
        ButtonSequence::prepare(&[DOUBLE[0], DOUBLE[0]]),
        Err(Refusal::UnbalancedPattern)
    ));
    assert!(matches!(
        ButtonSequence::prepare(&[Step {
            button_identity: "",
            pressed: true
        }]),
        Err(Refusal::ButtonIdentity)
    ));
}
