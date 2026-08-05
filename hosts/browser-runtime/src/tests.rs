use super::*;

fn completion(session: &BrowserSession, success: bool) -> Vec<u8> {
    let mut frame = session.expected_completion[..session.expected_completion_len].to_vec();
    frame.push(u8::from(success));
    frame
}

#[test]
fn exact_kernel_completion_advances_and_wrong_identity_fails_closed() {
    let mut session = BrowserSession::start(0).expect("browser kernel session starts");
    while session.effect_kind() != EFFECT_PRESENT {
        let exact = completion(&session, true);
        session
            .complete_current(&exact)
            .expect("timer completion advances");
    }
    assert_eq!(session.effect_kind(), EFFECT_PRESENT);
    let exact = completion(&session, true);
    session
        .complete_current(&exact)
        .expect("exact frame advances");
    assert_eq!(session.receipts, 1);
    let mut changed = completion(&session, true);
    changed[1] ^= 1;
    assert_eq!(
        session.complete_current(&changed),
        Err(ERROR_COMPLETION_IDENTITY)
    );
    assert!(session.terminal_failure);
}

#[test]
fn duplicate_completion_is_rejected_by_rust_runtime() {
    let mut session = BrowserSession::start(0).expect("browser kernel session starts");
    let exact = completion(&session, true);
    session
        .complete_current(&exact)
        .expect("first completion accepted");
    assert_eq!(
        session.complete_current(&exact),
        Err(ERROR_DUPLICATE_COMPLETION)
    );
}

#[test]
fn cancellation_and_platform_failure_are_honest_terminal_states() {
    let mut cancelled = BrowserSession::start(0).expect("browser kernel session starts");
    assert_eq!(cancelled.cancel(), Err(ERROR_CANCELLED));
    assert_eq!(cancelled.status(), ERROR_CANCELLED);
    assert!(cancelled.terminal_failure);

    let mut failed = BrowserSession::start(1).expect("browser kernel session starts");
    let failure = completion(&failed, false);
    assert_eq!(
        failed.complete_current(&failure),
        Err(ERROR_TERMINAL_FAILURE)
    );
    assert_eq!(failed.status(), ERROR_TERMINAL_FAILURE);
    assert!(failed.terminal_failure);
}

#[test]
fn evidence_exhaustion_is_a_distinct_terminal_failure() {
    assert_eq!(
        BrowserSession::start_with_evidence_limit(0, Some(1)).err(),
        Some(ERROR_EVIDENCE_EXHAUSTED)
    );
}

#[test]
fn capacities_are_sealed_before_activation_and_never_grow() {
    let mut session = BrowserSession::start(0).expect("browser kernel session starts");
    while session.status() == STATUS_RUNNING {
        assert_eq!(session.capacity_seal(), session.seal);
        let exact = completion(&session, true);
        session
            .complete_current(&exact)
            .expect("completion advances");
    }
    assert_eq!(session.status(), STATUS_COMPLETE);
    assert_eq!(session.receipts, MAXIMUM_RECEIPTS);
    assert_eq!(session.capacity_seal(), session.seal);
}

#[test]
fn exact_browser_fragment_uses_the_planned_item_and_byte_bounded_cord() {
    let session = BrowserSession::start(0).expect("browser kernel session starts");
    let lowered = lower_plan_fragment(&session.fragment).expect("exact fragment lowers");
    assert_eq!(lowered.cord_value_slots, 4);
    assert_eq!(lowered.cord_value_bytes, 64);
    assert_eq!(lowered.cords.len(), 1);
    assert_eq!(lowered.cords[0].spec.item_capacity, 4);
    assert_eq!(lowered.cords[0].spec.byte_capacity, 64);
    assert_eq!(session.scheduler.drivers().len(), 2);
    assert!(session.output_len <= FRAME_CAPACITY);
    assert!(session.expected_completion_len < FRAME_CAPACITY);
}

#[test]
fn host_identity_is_bounded_to_the_two_page_instances() {
    let first = BrowserSession::start(0).expect("first browser host starts");
    let second = BrowserSession::start(1).expect("second browser host starts");
    assert_ne!(first.fragment.host_id, second.fragment.host_id);
    assert_ne!(first.fragment.boot_id, second.fragment.boot_id);
    assert_ne!(first.active_play_id, second.active_play_id);
    assert!(matches!(BrowserSession::start(2), Err(ERROR_INVALID_HOST)));
}
