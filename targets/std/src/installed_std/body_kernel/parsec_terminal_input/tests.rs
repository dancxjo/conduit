//! Concrete owner/Mask return negatives with real instruction Source/effect ACK.
//! Key events and source scan acknowledgements are synthetic mechanism fixtures;
//! this is not production completed-event custody or whole-Body execution proof.
use super::super::prompt::tests::{accept, prepared_keys, Pending};
use super::*;
use conduit_kernel::{BoundedValueRef, HostCallId, NodeId, RequestId, ValueRef};

fn event(kind: InputRealization, epoch: u64) -> CompletedInputEvent {
    CompletedInputEvent {
        descriptor: 0,
        realization: kind,
        request: conduit_kernel::scheduler::HostCallRequest {
            node: NodeId(1),
            call: HostCallId(0),
            request: RequestId(1),
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
        value: ValueRef {
            slot: 1,
            generation: 1,
            byte_len: 3,
        },
        physical_event: 1,
        input_view_epoch: Some(epoch),
    }
}

#[test]
fn actual_terminal_return_refuses_stale_unmapped_and_release_then_waits_for_source_ack() {
    let (host, plan, play, identity, port, node) = super::super::tests::fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut output = Vec::new();
    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host.advertisement(), &plan, &play);
    accept(&mut keys, &requests);
    owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap();
    assert!(owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap());
    let initial = owner
        .current_face_show(&map, host.advertisement())
        .unwrap()
        .1
        .clone();
    let queue =
        BodyLiveForeQueue::for_parsec_plan(&plan.plots[0].plan, crate::RunControl::default(), 1)
            .unwrap();
    let mut encoder =
        conduit_semantic_catalog::PreparedButtonTransitionEncoder::new("button/primary").unwrap();
    let press = encoder.encode(true, 1).unwrap().to_vec();
    let release = encoder.encode(false, 2).unwrap().to_vec();
    for (kind, epoch, bytes) in [
        (
            InputRealization::RawKeyboard,
            initial.show.play_sequence,
            vec![0x2c, 0, 0],
        ),
        (
            InputRealization::RawKeyboard,
            initial.show.play_sequence,
            vec![0x28, 0, 0],
        ),
        (
            InputRealization::SpaceButton,
            initial.show.play_sequence + 1,
            press.clone(),
        ),
        (
            InputRealization::SpaceButton,
            initial.show.play_sequence,
            release,
        ),
    ] {
        assert!(!owner
            .admit_completed_input(
                &map,
                host.advertisement(),
                &event(kind, epoch),
                &bytes,
                &queue
            )
            .unwrap());
        assert_eq!(queue.status().queue_accepted, 0);
        assert_eq!(
            owner
                .current_face_show(&map, host.advertisement())
                .unwrap()
                .1,
            &initial
        );
    }
    let fire = event(InputRealization::SpaceButton, initial.show.play_sequence);
    assert!(owner
        .admit_completed_input(&map, host.advertisement(), &fire, &press, &queue)
        .unwrap());
    assert_eq!(queue.status().queue_accepted, 1);
    assert!(owner
        .input_view_epoch(&map, host.advertisement())
        .unwrap()
        .is_none());
    assert!(!owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap());
    assert!(!owner
        .admit_completed_input(&map, host.advertisement(), &fire, &press, &queue)
        .unwrap());
    // This test's acknowledgement is source-evaluator mechanism evidence, not whole-Body execution.
    let sources = crate::flow_activation::ParsecSourcePlans::prepare(host.advertisement()).unwrap();
    let mut evaluator = sources.prepare_evaluator().unwrap();
    let command = ParsecMapCommand::new("game/start".into(), 1, false)
        .unwrap()
        .encode()
        .unwrap();
    let result = evaluator
        .evaluate(map.current_state().unwrap(), &command)
        .unwrap()
        .result_bytes()
        .to_vec();
    map.acknowledge_lowered(&port, node, 0, &result).unwrap();
    assert!(owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap());
    assert!(owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap());
    let (face, current) = owner.current_face_show(&map, host.advertisement()).unwrap();
    assert_eq!(face.actions[0].identity, "game/shoot");
    let current = current.clone();
    // The fixture has not drained queue slot: checked Shoot return must honestly refuse Full.
    assert!(!owner
        .admit_completed_input(
            &map,
            host.advertisement(),
            &event(InputRealization::SpaceButton, current.show.play_sequence),
            &press,
            &queue
        )
        .unwrap());
    assert_eq!(queue.status().queue_accepted, 1);
    assert!(owner
        .input_view_epoch(&map, host.advertisement())
        .unwrap()
        .is_none());
    assert!(owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap());
    assert!(owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap());
    assert_ne!(
        owner
            .current_face_show(&map, host.advertisement())
            .unwrap()
            .1
            .show_id,
        current.show_id
    );
    let remap = ParsecMapCommand::new("game/remap".into(), 2, true)
        .unwrap()
        .encode()
        .unwrap();
    let remapped = evaluator
        .evaluate(map.current_state().unwrap(), &remap)
        .unwrap()
        .result_bytes()
        .to_vec();
    map.acknowledge_lowered(&port, node, 1, &remapped).unwrap();
    assert!(owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap());
    assert!(owner
        .sync_prompt(&map, host.advertisement(), &keys, &mut output)
        .unwrap());
    let epoch = owner
        .input_view_epoch(&map, host.advertisement())
        .unwrap()
        .unwrap();
    for (kind, captured, bytes) in [
        (InputRealization::SpaceButton, epoch, press.clone()),
        (InputRealization::RawKeyboard, epoch, vec![0x28, 1, 0]),
        (InputRealization::RawKeyboard, epoch - 1, vec![0x28, 0, 0]),
    ] {
        assert!(!owner
            .admit_completed_input(
                &map,
                host.advertisement(),
                &event(kind, captured),
                &bytes,
                &queue
            )
            .unwrap());
        assert_eq!(queue.status().queue_accepted, 1);
        assert_eq!(
            owner.input_view_epoch(&map, host.advertisement()).unwrap(),
            Some(epoch)
        );
    }
    assert_eq!(
        map.current_state().unwrap(),
        ParsecMapState::new(true, 3, true)
            .unwrap()
            .encode()
            .unwrap()
    );
}
