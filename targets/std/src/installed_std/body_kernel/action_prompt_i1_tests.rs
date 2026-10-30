//! Current Start fallback through source policy, with request-acceptance and
//! completed-event mechanism fixtures. No physical controller proof is claimed.
use super::tests::{prepared_keys, Pending};
use super::*;
use crate::installed_std::{
    keyboard_input_completion::CompletedInputEvent, keyboard_input_host::InputRequestKind,
};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
use conduit_kernel::ValueRef;

#[test]
fn action_prompt_i1_current_keyboard_fallback_retains_start_and_refuses_stale_epoch() {
    let (host, plan, play, identity, _, _) = super::super::tests::fixture();
    let host = host.advertisement();
    let mut map = ParsecAcknowledgedMap::prepare(&plan, host, &play, &identity).unwrap();
    map.confirm(host, &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host, 80, 24).unwrap();
    let mut output = Vec::new();
    owner.ensure_present(&map, host, &mut output).unwrap();
    let original = owner.current_face_show(&map, host).unwrap().0.clone();
    assert_eq!(original.actions[0].identity, "game/start");
    let before = map.current_state().unwrap().to_vec();
    assert!(!ParsecMapState::decode(&before).unwrap().keyboard());

    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host, &plan, &play);
    // The selected Fire realization is unavailable. The actual checked policy
    // may choose a current keyboard scope without rewriting preferred map state.
    let (descriptor, (request, kind)) = requests
        .iter()
        .enumerate()
        .find(|(_, (_, kind))| matches!(kind, InputRequestKind::Keyboard))
        .unwrap();
    assert!(!owner.sync_prompt(&map, host, &keys, &mut output).unwrap());
    assert!(owner
        .qualify_instruction(&map, host, &keys)
        .unwrap()
        .is_none());
    keys.accept(*request, &[], *kind).unwrap();
    assert!(keys
        .require_current_input_scope(InputRealization::RawKeyboard, &plan, &play, host)
        .is_ok());
    assert!(keys
        .require_current_input_scope(InputRealization::SpaceButton, &plan, &play, host)
        .is_err());
    assert!(owner.sync_prompt(&map, host, &keys, &mut output).unwrap());
    let observed = owner
        .instruction_observation(&map, host, &keys)
        .unwrap()
        .unwrap();
    assert!(observed.prompt_text().contains("Input: Enter to Begin."));
    assert!(!observed.prompt_text().contains("Fire"));
    assert_eq!(observed.binding().control_identity(), "keyboard/enter");
    assert_eq!(
        observed.binding().offered_action().action_identity(),
        "game/start"
    );
    assert_eq!(observed.face(), &original);
    assert_eq!(map.current_state().unwrap(), before);
    let epoch = observed.input_view_epoch();
    let bytes = KeyEvent::new(0x28, KeyTransition::Pressed, KeyModifiers::NONE)
        .unwrap()
        .encode();
    let mut event = CompletedInputEvent {
        descriptor,
        realization: InputRealization::RawKeyboard,
        request: *request,
        value: ValueRef {
            slot: 1,
            generation: 1,
            byte_len: bytes.len() as u32,
        },
        physical_event: 1,
        input_view_epoch: Some(epoch + 1),
    };
    let queue = crate::BodyLiveForeQueue::for_parsec_plan(
        &plan.plots[0].plan,
        crate::RunControl::default(),
        1,
    )
    .unwrap();
    assert!(!owner
        .admit_completed_input(&map, host, &event, &bytes, &queue)
        .unwrap());
    assert_eq!(queue.status().queue_accepted, 0);
    event.input_view_epoch = Some(epoch);
    // This private event is synthetic; production requires completed-event
    // custody and authorize_prompted_event before entering this typed return.
    assert!(owner
        .admit_completed_input(&map, host, &event, &bytes, &queue)
        .unwrap());
    assert_eq!(queue.status().queue_accepted, 1);
    assert_eq!(map.current_state().unwrap(), before);
    owner.sync_prompt(&map, host, &keys, &mut output).unwrap();
    assert!(std::str::from_utf8(&output)
        .unwrap()
        .ends_with("Input unavailable.\r\n"));
}

#[test]
fn action_prompt_i1_fallback_witness_refuses_cancel_before_instruction_effect() {
    let (host, plan, play, identity, _, _) = super::super::tests::fixture();
    let host = host.advertisement();
    let mut map = ParsecAcknowledgedMap::prepare(&plan, host, &play, &identity).unwrap();
    map.confirm(host, &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host, 80, 24).unwrap();
    let mut output = Vec::new();
    owner.ensure_present(&map, host, &mut output).unwrap();
    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host, &plan, &play);
    let (request, kind) = requests
        .iter()
        .find(|(_, kind)| matches!(kind, InputRequestKind::Keyboard))
        .unwrap();
    keys.accept(*request, &[], *kind).unwrap();
    let witness = owner
        .qualify_instruction(&map, host, &keys)
        .unwrap()
        .unwrap();
    let before = output.len();
    keys.cancel();
    assert!(owner
        .revalidate_instruction(witness, &map, host, &keys)
        .is_err());
    assert!(!owner.sync_prompt(&map, host, &keys, &mut output).unwrap());
    assert_eq!(output.len(), before);
    assert!(owner
        .instruction_observation(&map, host, &keys)
        .unwrap()
        .is_none());
}
