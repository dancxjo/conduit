use super::*;
use conduit_human::{KeyModifiers, KeyTransition};

#[test]
fn host_semantics_share_the_core_state_machine_and_refuse_malformed_input() {
    let mut map = Some(
        PreparedKeymap::new(
            &conduit_human::default_keymap_configuration(),
            &conduit_human::default_choices_configuration(),
        )
        .unwrap(),
    );
    let event = KeyEvent::new(0x22, KeyTransition::Pressed, KeyModifiers::RIGHT_ALT).unwrap();
    let output = execute_host(true, &mut map, &event.encode())
        .unwrap()
        .unwrap();
    assert_eq!(output.as_slice(), "€".as_bytes());
    assert_eq!(execute_host(true, &mut map, &[0, 1]), Err(invalid_input(1)));

    let ctrl_g = KeyEvent::new(0x0a, KeyTransition::Pressed, KeyModifiers::LEFT_CONTROL).unwrap();
    let chord = execute_host(false, &mut map, &ctrl_g.encode())
        .unwrap()
        .unwrap();
    assert_eq!(
        ChordInfo::decode(chord.as_slice()).unwrap().chord_id(),
        conduit_human::CoreChordId::CancelOrEscape
    );
}

#[test]
fn prepared_layouts_keep_dead_key_state_per_node_and_emit_bounded_fallback_text() {
    let layout = conduit_human::default_keymap_configuration();
    let mut first = Some(
        PreparedKeymap::new(&layout, &conduit_human::default_choices_configuration()).unwrap(),
    );
    let mut second = Some(
        PreparedKeymap::new(&layout, &conduit_human::default_choices_configuration()).unwrap(),
    );
    let acute = KeyEvent::new(0x34, KeyTransition::Pressed, KeyModifiers::RIGHT_ALT).unwrap();
    let a = KeyEvent::new(0x04, KeyTransition::Pressed, KeyModifiers::from_bits(0)).unwrap();
    assert_eq!(
        execute_host(true, &mut first, &acute.encode()).unwrap(),
        None
    );
    assert_eq!(
        execute_host(true, &mut second, &a.encode())
            .unwrap()
            .unwrap()
            .as_slice(),
        b"a"
    );
    assert_eq!(
        execute_host(true, &mut first, &a.encode())
            .unwrap()
            .unwrap()
            .as_slice(),
        "á".as_bytes()
    );
    assert_eq!(
        execute_host(true, &mut first, &acute.encode()).unwrap(),
        None
    );
    let digit = KeyEvent::new(0x1e, KeyTransition::Pressed, KeyModifiers::from_bits(0)).unwrap();
    assert_eq!(
        execute_host(true, &mut first, &digit.encode())
            .unwrap()
            .unwrap()
            .as_slice(),
        "´1".as_bytes()
    );
    assert_eq!(
        execute_host(true, &mut None, &a.encode()),
        Err(invalid_input(6))
    );
}

#[test]
fn cancellation_clears_one_pending_semantic_request_without_a_duplicate() {
    let mut operation = InputSemanticBack {
        pending: None,
        next: 0,
        selections: false,
        selection: None,
    };
    let value = conduit_kernel::ValueRef {
        slot: 1,
        generation: 1,
        byte_len: KEY_EVENT_ENCODED_LEN as u32,
    };
    let mut io = StepIo::test_frame([Some(value)], [false], [Some(4)], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(0))
    );
    StepBack::<1>::cancel(&mut operation);
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(io.test_consumed_closed(PortId(0)));
}

#[test]
fn keymap_selection_uses_canonical_call_zero_before_key_call_one() {
    let mut back = InputSemanticBack {
        pending: None,
        next: 0,
        selections: true,
        selection: None,
    };
    let key = conduit_kernel::ValueRef {
        slot: 1,
        generation: 1,
        byte_len: KEY_EVENT_ENCODED_LEN as u32,
    };
    let selection = conduit_kernel::ValueRef {
        slot: 2,
        generation: 1,
        byte_len: 2,
    };
    let mut io = StepIo::test_frame(
        [Some(key), Some(selection)],
        [false; 2],
        [Some(4), None],
        None,
        8,
    );
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_host_request().unwrap().1, HostCallId(0));
    StepBack::<2>::cancel(&mut back);
    let mut io = StepIo::test_frame([Some(key), None], [false; 2], [Some(4), None], None, 8);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_host_request().unwrap().1, HostCallId(1));
}
