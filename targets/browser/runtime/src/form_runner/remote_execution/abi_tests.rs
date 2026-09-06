use super::*;

fn put(value: &Value) -> usize {
    let bytes = serde_json::to_vec(value).unwrap();
    assert!(bytes.len() <= INPUT_BYTES);
    INPUT.with(|input| input.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
    bytes.len()
}

fn output() -> Value {
    OUTPUT.with(|output| {
        serde_json::from_slice(&output.borrow()[..conduit_browser_remote_output_len()]).unwrap()
    })
}

fn request() -> (Value, ActivePlayId) {
    let (plan, host, _, binding) = super::super::tests::fixture();
    let mut hello = vec![0; HELLO_BYTES];
    let length = conduit_wire::encode_session_frame_into(
        binding.hello_frame(),
        &mut hello,
        binding.limits.maximum_payload_bytes,
        HELLO_BYTES as u32,
    )
    .unwrap();
    hello.truncate(length);
    let observations = super::super::tests::observations(&host);
    (
        json!({"plan":plan,"host":host,"hello":hello,"active_play_id":binding.source_active_play_id,"observations":observations}),
        binding.source_active_play_id,
    )
}

#[test]
fn abi_retains_pending_identity_and_resources_until_exact_cancel_ack() {
    let (request, play) = request();
    assert!(!is_active());
    assert_eq!(conduit_browser_remote_start(put(&request)), READY);
    assert!(is_active());
    let initial = output();
    assert_eq!(
        initial["progress"]["effect"]["effect_kind"],
        "button-transition"
    );
    let placement = initial["progress"]["placement_id"].clone();
    let sequence = initial["progress"]["request_sequence"].clone();
    assert_eq!(
        conduit_browser_remote_command(put(&json!({"kind":"Poll","play":play}))),
        READY
    );
    assert_eq!(output(), initial);
    assert_eq!(conduit_browser_remote_start(put(&request)), ERROR);
    assert_eq!(
        conduit_browser_remote_command(put(
            &json!({"kind":"Complete","play":"stale","placement_id":placement,"request":sequence,"output":[]})
        )),
        ERROR
    );
    assert!(is_active());
    assert_eq!(
        conduit_browser_remote_command(put(
            &json!({"kind":"Complete","play":play,"placement_id":placement,"request":999,"output":[]})
        )),
        ERROR
    );
    assert_eq!(
        conduit_browser_remote_command(put(&json!({"kind":"Poll","play":play}))),
        READY
    );
    assert_eq!(output(), initial);
    assert_eq!(
        crate::form_runner::abi::conduit_tour_start(1, 1, 1, 0),
        -402
    );
    assert_eq!(
        crate::form_runner::abi::conduit_tour_start_recursive(1, 1, 1, 0),
        -402
    );
    assert!(is_active());
    assert_eq!(crate::form_runner::abi::conduit_browser_body_start(1), -402);
    assert_eq!(
        crate::form_runner::multihost::conduit_tour_multi_start_source(1, 1, 1, 1, 1, 0),
        -452
    );
    assert_eq!(
        crate::form_runner::multihost::conduit_tour_multi_start_sink(1, 1, 1, 0),
        -452
    );
    assert_eq!(
        conduit_browser_remote_command(put(&json!({"kind":"Cancel","play":play}))),
        READY
    );
    assert_eq!(output()["progress"]["effect"]["effect_kind"], "cancel");
    assert!(is_active());
    assert_eq!(
        conduit_browser_remote_command(put(
            &json!({"kind":"AcknowledgeCancel","play":play,"placement_id":"wrong","request":sequence})
        )),
        ERROR
    );
    assert!(is_active());
    assert_eq!(
        conduit_browser_remote_command(put(
            &json!({"kind":"AcknowledgeCancel","play":play,"placement_id":placement,"request":sequence})
        )),
        READY
    );
    assert_eq!(output()["progress"]["disposition"], "cancelled");
    assert!(conduit_browser_remote_output_len() < OUTPUT_BYTES);
    assert!(!is_active());
}

#[test]
fn malformed_start_and_commands_do_not_discard_live_slot() {
    let (mut request, play) = request();
    let original = request["hello"].clone();
    request["hello"] = json!([0, 1, 2]);
    assert_eq!(conduit_browser_remote_start(put(&request)), ERROR);
    assert!(!is_active());
    request["hello"] = original;
    assert_eq!(conduit_browser_remote_start(put(&request)), READY);
    assert_eq!(conduit_browser_remote_command(INPUT_BYTES + 1), ERROR);
    assert!(is_active());
    let pending = SLOT.with(|slot| slot.borrow_mut().as_mut().unwrap().progress().unwrap());
    assert_eq!(
        conduit_browser_remote_command(put(&json!({"kind":"Cancel","play":play}))),
        READY
    );
    assert_eq!(
        conduit_browser_remote_command(put(
            &json!({"kind":"AcknowledgeCancel","play":play,"placement_id":pending["progress"]["placement_id"],"request":pending["progress"]["request_sequence"]})
        )),
        READY
    );
    assert!(!is_active());
    assert!(conduit_browser_remote_output_len() <= OUTPUT_BYTES);
}
