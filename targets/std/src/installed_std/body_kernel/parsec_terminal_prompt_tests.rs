//! Real terminal write/flush, checked policy and exact accepted-origin fixture.
//! Scheduler request issuance remains the whole-Body integration test's proof.
use super::*;
use crate::installed_std::{
    keyboard_input_binding::KeyboardRequestAdmission, keyboard_input_host::InputRequestKind,
};
use conduit_kernel::{scheduler::HostCallRequest, BoundedValueRef, RequestId, ValueRef};
use conduit_plan_lowering::{
    fragment_set::{lower_local_fragment_set_from_plans, FragmentSetBounds},
    lowering::FIXED_KERNEL_STORAGE_PROFILE,
};

pub(in crate::installed_std::body_kernel::parsec_terminal_owner) struct Pending;
impl crate::hosted_keyboard::HostedKeyboardAdapter for Pending {
    fn poll_next(&mut self) -> crate::hosted_keyboard::HostedKeyboardPoll {
        crate::hosted_keyboard::HostedKeyboardPoll::Pending
    }
}
pub(in crate::installed_std::body_kernel::parsec_terminal_owner) fn prepared_keys<'a>(
    adapter: &'a mut Pending,
    host: &HostAdvertisement,
    plan: &BodyPlan,
    play: &BodyPlayIdentity,
) -> (
    KeyboardInputHost<'a>,
    Vec<(HostCallRequest, InputRequestKind)>,
) {
    let lowered = lower_local_fragment_set_from_plans(
        &[&plan.plots[0].plan],
        FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: 1,
            nodes: crate::installed_std::MAX_NODES as u16,
            cords: crate::installed_std::MAX_CORDS as u16,
            queue_slots: crate::installed_std::MAX_QUEUE_SLOTS as u16,
            value_bytes: 16 * 1024 * 1024,
            sign_items: 4096,
            sign_bytes: 1024 * 1024,
        },
    )
    .unwrap();
    let operations = lowered.partitions[0].host_calls.clone();
    let mut admission = KeyboardRequestAdmission::prepare_body(
        host,
        plan,
        play,
        &[lowered.partitions[0].identity.clone()],
        &operations,
    )
    .unwrap();
    admission.enable_completed_event_custody().unwrap();
    admission.confirm_body_play(host, play).unwrap();
    let mut keys =
        KeyboardInputHost::new(Some(adapter), operations.iter().map(|op| &op.contract_id));
    keys.install_request_admission(admission).unwrap();
    let requests = operations
        .into_iter()
        .filter_map(|op| {
            let kind = match InputRealization::from_contract(op.contract_id.as_str())? {
                InputRealization::RawKeyboard => InputRequestKind::Keyboard,
                InputRealization::SpaceButton => InputRequestKind::SpaceButton,
            };
            Some((
                HostCallRequest {
                    node: op.node,
                    call: op.call,
                    request: RequestId(0),
                    input: BoundedValueRef::new(
                        ValueRef {
                            slot: 0,
                            generation: 1,
                            byte_len: 0,
                        },
                        0,
                    )
                    .unwrap(),
                },
                kind,
            ))
        })
        .collect();
    (keys, requests)
}
pub(in crate::installed_std::body_kernel::parsec_terminal_owner) fn accept(
    keys: &mut KeyboardInputHost<'_>,
    requests: &[(HostCallRequest, InputRequestKind)],
) {
    for (request, kind) in requests {
        keys.accept(*request, &[], *kind).unwrap();
    }
}

#[test]
fn only_exact_accepted_request_enables_source_space_prompt_and_epoch() {
    let (host, plan, play, identity, _, _) = super::super::tests::fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut output = Vec::new();
    owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap();
    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host.advertisement(), &plan, &play);
    let before = output.len();
    assert!(!owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap());
    assert_eq!(output.len(), before);
    assert_eq!(
        owner
            .prompt_input_view_epoch(&map, host.advertisement(), &keys)
            .unwrap(),
        None
    );
    accept(&mut keys, &requests);
    assert!(owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap());
    assert!(std::str::from_utf8(&output)
        .unwrap()
        .contains("Input: Space to Begin."));
    assert!(owner
        .prompt_input_view_epoch(&map, host.advertisement(), &keys)
        .unwrap()
        .is_some());
    keys.cancel();
    assert!(owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap());
    assert!(std::str::from_utf8(&output)
        .unwrap()
        .ends_with("Input unavailable.\r\n"));
    assert_eq!(
        owner
            .prompt_input_view_epoch(&map, host.advertisement(), &keys)
            .unwrap(),
        None
    );
}

#[test]
fn remap_withdraws_space_and_requires_new_show_before_source_enter_instruction() {
    let (host, plan, play, identity, port, node) = super::super::tests::fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut output = Vec::new();
    owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap();
    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host.advertisement(), &plan, &play);
    accept(&mut keys, &requests);
    owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap();
    let old_epoch = owner
        .prompt_input_view_epoch(&map, host.advertisement(), &keys)
        .unwrap()
        .unwrap();
    let sources = crate::flow_activation::ParsecSourcePlans::prepare(host.advertisement()).unwrap();
    let mut evaluator = sources.prepare_evaluator().unwrap();
    let command = conduit_face::ParsecMapCommand::new("game/remap".into(), 1, true)
        .unwrap()
        .encode()
        .unwrap();
    let result = evaluator
        .evaluate(map.current_state().unwrap(), &command)
        .unwrap()
        .result_bytes()
        .to_vec();
    map.acknowledge_lowered(&port, node, 0, &result).unwrap();
    let start = output.len();
    owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap();
    assert!(std::str::from_utf8(&output[start..])
        .unwrap()
        .contains("Input unavailable."));
    owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap();
    assert!(std::str::from_utf8(&output[start..])
        .unwrap()
        .contains("Input: Enter to Begin."));
    assert_ne!(
        owner
            .prompt_input_view_epoch(&map, host.advertisement(), &keys)
            .unwrap()
            .unwrap(),
        old_epoch
    );
}

#[test]
fn failed_prompt_flush_retires_availability_and_stale_host_visibly_withdraws() {
    struct FailsFlush;
    impl Write for FailsFlush {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }
    let (host, plan, play, identity, _, _) = super::super::tests::fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host.advertisement(), &plan, &play);
    accept(&mut keys, &requests);
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    owner
        .ensure_present(&map, host.advertisement(), &mut Vec::new())
        .unwrap();
    assert!(owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut FailsFlush)
        .is_err());
    assert_eq!(
        owner
            .prompt_input_view_epoch(&map, host.advertisement(), &keys)
            .unwrap(),
        None
    );
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut output = Vec::new();
    owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap();
    owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap();
    let mut changed = host.advertisement().clone();
    changed.offer_generation.0 += 1;
    assert!(owner
        .sync_prompt(&map, &changed, &keys, &mut output)
        .is_err());
    assert!(std::str::from_utf8(&output)
        .unwrap()
        .ends_with("Input unavailable.\r\n"));
    assert_eq!(
        owner
            .prompt_input_view_epoch(&map, host.advertisement(), &keys)
            .unwrap(),
        None
    );
}

#[test]
fn flushed_prompt_authorizes_only_current_source_control_and_consistent_typed_return() {
    use crate::installed_std::keyboard_input_completion::CompletedInputEvent;
    let (host, plan, play, identity, _, _) = super::super::tests::fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut output = Vec::new();
    owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap();
    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host.advertisement(), &plan, &play);
    accept(&mut keys, &requests);
    owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap();
    let epoch = owner
        .prompt_input_view_epoch(&map, host.advertisement(), &keys)
        .unwrap()
        .unwrap();
    let (descriptor, (request, _)) = requests
        .iter()
        .enumerate()
        .find(|(_, (_, kind))| matches!(kind, InputRequestKind::SpaceButton))
        .unwrap();
    let bytes = conduit_semantic_catalog::button_transition_value("button/primary", true, 0)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    // Private completed-event correlation fixture; actual scheduler minting
    // and Fore acknowledgement are exercised by the whole-Body runtime test.
    let mut event = CompletedInputEvent {
        descriptor,
        realization: InputRealization::SpaceButton,
        request: *request,
        value: ValueRef {
            slot: 1,
            generation: 1,
            byte_len: bytes.len() as u32,
        },
        physical_event: 0,
        input_view_epoch: Some(epoch + 1),
    };
    assert!(!owner
        .authorize_prompted_event(&map, host.advertisement(), &keys, &event)
        .unwrap());
    event.input_view_epoch = Some(epoch);
    assert!(owner
        .authorize_prompted_event(&map, host.advertisement(), &keys, &event)
        .unwrap());
    let queue = crate::BodyLiveForeQueue::for_parsec_plan(
        &plan.plots[0].plan,
        crate::RunControl::default(),
        1,
    )
    .unwrap();
    assert!(owner
        .admit_completed_input(&map, host.advertisement(), &event, &bytes, &queue)
        .unwrap());
    assert_eq!(queue.status().queue_accepted, 1);
    owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap();
    assert!(std::str::from_utf8(&output)
        .unwrap()
        .ends_with("Input unavailable.\r\n"));
    assert_eq!(
        owner
            .prompt_input_view_epoch(&map, host.advertisement(), &keys)
            .unwrap(),
        None
    );
}

#[test]
fn read_only_instruction_observation_exists_only_after_exact_scope_and_flush() {
    let (host, plan, play, identity, _, _) = super::super::tests::fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut output = Vec::new();
    owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap();
    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host.advertisement(), &plan, &play);
    assert!(owner
        .instruction_observation(&map, host.advertisement(), &keys)
        .unwrap()
        .is_none());
    accept(&mut keys, &requests);
    assert!(owner
        .instruction_observation(&map, host.advertisement(), &keys)
        .unwrap()
        .is_none());
    owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap();
    let observation = owner
        .instruction_observation(&map, host.advertisement(), &keys)
        .unwrap()
        .unwrap();
    assert_eq!(observation.binding().control_label(), "Space");
    assert_eq!(
        observation.binding().offered_action().action_identity(),
        "game/start"
    );
    assert_eq!(observation.face().actions[0].identity, "game/start");
    assert_eq!(
        observation.show().show.play_sequence,
        observation.input_view_epoch()
    );
    let serialized = serde_json::to_value(&observation).unwrap();
    assert_eq!(
        serialized["body_play"]["active_play_id"],
        play.active_play_id.as_str()
    );
    let mut source =
        crate::flow_activation::ParsecInputInstructionSourcePlan::prepare(host.advertisement())
            .unwrap()
            .evaluator()
            .unwrap();
    let binding = observation.binding().clone().encode().unwrap();
    let candidate = source
        .evaluate(
            map.current_state().unwrap(),
            &binding,
            conduit_face::ParsecInputInstructionLook::Linear,
            false,
        )
        .unwrap();
    assert_eq!(
        observation.effect().canonical_instruction(),
        candidate.canonical_bytes()
    );
    assert_eq!(
        observation.effect().instruction_producer_plan(),
        candidate.producer_plan_id()
    );
    assert_eq!(
        observation.effect().instruction_producer_plot(),
        candidate.producer_plot()
    );
    assert_eq!(
        serialized["effect"]["value_bytes"],
        candidate.canonical_bytes().len()
    );
    assert_ne!(
        serialized["effect"]["source_node"],
        serialized["effect"]["effect_node"]
    );
    assert_eq!(serialized["effect"]["source_request"], 0);
    assert_eq!(serialized["effect"]["effect_request"], 0);
    assert!(observation.prompt_text().contains("Input: Space to Begin."));
    keys.cancel();
    assert!(owner
        .instruction_observation(&map, host.advertisement(), &keys)
        .unwrap()
        .is_none());
}

#[test]
fn instruction_witness_requires_current_accepted_scope_and_exact_mask_play() {
    let (host, plan, play, identity, _, _) = super::super::tests::fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    owner
        .ensure_present(&map, host.advertisement(), &mut Vec::new())
        .unwrap();
    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host.advertisement(), &plan, &play);
    assert!(owner
        .qualify_instruction(&map, host.advertisement(), &keys)
        .unwrap()
        .is_none());
    accept(&mut keys, &requests);
    let request = owner
        .execution
        .pending_instruction_request(host.advertisement())
        .unwrap();
    assert_eq!(request.request, RequestId(0));
    assert!(request.input.value.byte_len > 0);
    let witness = owner
        .qualify_instruction(&map, host.advertisement(), &keys)
        .unwrap()
        .unwrap();
    assert_eq!(witness.map_bytes(), map.current_state().unwrap());
    assert!(
        *ActionInputBindingProposalInfo::decode(witness.binding_bytes())
            .unwrap()
            .available()
    );
    let mask = witness.show.planned_mask.clone();
    let active = conduit_core::bind_active_play(
        &mask.plan.plan_id,
        &host.advertisement().host_id,
        &host.advertisement().boot_id,
        witness.show.show.play_sequence,
    );
    witness
        .require_source_scope(&plan, &play, &mask, &active, host.advertisement())
        .unwrap();
    let mut wrong = active.clone();
    wrong.play_sequence += 1;
    assert!(witness
        .require_source_scope(&plan, &play, &mask, &wrong, host.advertisement())
        .is_err());
    let mut changed = host.advertisement().clone();
    changed.offer_generation.0 += 1;
    assert!(witness
        .require_source_scope(&plan, &play, &mask, &active, &changed)
        .is_err());
    let witness = owner
        .revalidate_instruction(witness, &map, host.advertisement(), &keys)
        .unwrap();
    keys.cancel();
    assert!(owner
        .revalidate_instruction(witness, &map, host.advertisement(), &keys)
        .is_err());
}
