//! I1 counterexample, not completed ActionPrompt acceptance. Request acceptance
//! and source ACKs below are mechanism fixtures; no physical controller is used.
use super::tests::{prepared_keys, Pending};
use super::*;
use crate::installed_std::{
    keyboard_input_completion::CompletedInputEvent, keyboard_input_host::InputRequestKind,
};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
use conduit_kernel::ValueRef;

#[test]
fn action_prompt_i1_keyboard_reachable_but_fire_selected_has_no_alternate_prompt() {
    let (host, plan, play, identity, port, node) = super::super::tests::fixture();
    let host = host.advertisement();
    let mut map = ParsecAcknowledgedMap::prepare(&plan, host, &play, &identity).unwrap();
    map.confirm(host, &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host, 80, 24).unwrap();
    let mut output = Vec::new();
    owner.ensure_present(&map, host, &mut output).unwrap();
    let original = owner.current_face_show(&map, host).unwrap().0.clone();
    assert_eq!(original.actions[0].identity, "game/start");
    let before = map.current_state().unwrap().to_vec();
    let state = ParsecMapState::decode(&before).unwrap();
    assert!(!state.keyboard());

    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host, &plan, &play);
    // Accept keyboard only. The selected Fire realization has no current request.
    let (descriptor, (request, kind)) = requests
        .iter()
        .enumerate()
        .find(|(_, (_, kind))| matches!(kind, InputRequestKind::Keyboard))
        .unwrap();
    keys.accept(*request, &[], *kind).unwrap();
    assert!(keys
        .require_current_input_scope(InputRealization::RawKeyboard, &plan, &play, host)
        .is_ok());
    assert!(keys
        .require_current_input_scope(InputRealization::SpaceButton, &plan, &play, host)
        .is_err());
    let emitted = output.len();
    assert!(!owner.sync_prompt(&map, host, &keys, &mut output).unwrap());
    assert_eq!(output.len(), emitted);
    assert!(owner
        .qualify_instruction(&map, host, &keys)
        .unwrap()
        .is_none());
    assert_eq!(
        owner.prompt_input_view_epoch(&map, host, &keys).unwrap(),
        None
    );
    assert!(owner
        .instruction_observation(&map, host, &keys)
        .unwrap()
        .is_none());

    // Even a fabricated current Show epoch cannot turn reachable Enter into the
    // source-selected Fire. The ordinary checked return refuses before effects.
    let epoch = owner
        .current_face_show(&map, host)
        .unwrap()
        .1
        .show
        .play_sequence;
    let bytes = KeyEvent::new(0x28, KeyTransition::Pressed, KeyModifiers::NONE)
        .unwrap()
        .encode();
    let event = CompletedInputEvent {
        descriptor,
        realization: InputRealization::RawKeyboard,
        request: *request,
        value: ValueRef {
            slot: 1,
            generation: 1,
            byte_len: bytes.len() as u32,
        },
        physical_event: 1,
        input_view_epoch: Some(epoch),
    };
    let queue = crate::BodyLiveForeQueue::for_parsec_plan(
        &plan.plots[0].plan,
        crate::RunControl::default(),
        1,
    )
    .unwrap();
    assert!(!owner
        .authorize_prompted_event(&map, host, &keys, &event)
        .unwrap());
    assert!(!owner
        .admit_completed_input(&map, host, &event, &bytes, &queue)
        .unwrap());
    assert_eq!(queue.status().queue_accepted, 0);
    assert_eq!(map.current_state().unwrap(), before);
    assert_eq!(owner.current_face_show(&map, host).unwrap().0, &original);

    // Positive control: an explicit checked source remap and its ACK, rather
    // than presentation wording, establishes the alternative binding.
    let sources = crate::flow_activation::ParsecSourcePlans::prepare(host).unwrap();
    let mut evaluator = sources.prepare_evaluator().unwrap();
    let command = conduit_face::ParsecMapCommand::new("game/remap".into(), 1, true)
        .unwrap()
        .encode()
        .unwrap();
    let result = evaluator
        .evaluate(&before, &command)
        .unwrap()
        .result_bytes()
        .to_vec();
    map.acknowledge_lowered(&port, node, 0, &result).unwrap();
    owner.ensure_present(&map, host, &mut output).unwrap();
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
    assert_eq!(observed.face().basis.body_id, original.basis.body_id);
    assert_ne!(observed.input_view_epoch(), epoch);
    let witness = owner
        .qualify_instruction(&map, host, &keys)
        .unwrap()
        .unwrap();
    keys.cancel();
    assert!(owner
        .revalidate_instruction(witness, &map, host, &keys)
        .is_err());
    eprintln!("I1 COUNTEREXAMPLE: accepted Enter + selected unavailable Fire => no alternate prompt, no witness, no input effect; only explicit source-remap ACK enables Enter. Cross-Mask current custody remains unproved.");
}
