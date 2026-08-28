use conduit_browser_runtime::text_lab_split::BrowserTextLabFragment;
use conduit_core::{Plan, PlanFragment};
use conduit_semantic_catalog::{
    exact_text_lab_split_plan, TEXT_LAB_BROWSER_HOST, TEXT_LAB_FORWARD_LINE,
    TEXT_LAB_MAXIMUM_VALUES, TEXT_LAB_NATIVE_HOST, TEXT_LAB_RETURN_LINE,
};
use conduit_std_host::text_lab_split::NativeTextLabFragment;
use conduit_wire::{
    SessionBinding, SessionMachine, SessionMessage, SessionRole, SessionTerminalDisposition,
};

fn fragment<'a>(plan: &'a Plan, host: &str) -> &'a PlanFragment {
    plan.fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == host)
        .expect("exact Text Lab fragment")
}

fn binding(plan: &Plan, source_host: &str, sink_host: &str, line_id: &str) -> SessionBinding {
    let source = fragment(plan, source_host);
    let sink = fragment(plan, sink_host);
    let connection = source
        .connections
        .iter()
        .find(|connection| {
            connection
                .selected_line
                .as_ref()
                .is_some_and(|line| line.line_id.as_str() == line_id)
        })
        .expect("exact Text Lab planned connection");
    SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source.fragment_id.clone(),
        sink.fragment_id.clone(),
        connection,
    )
    .expect("exact Text Lab session binding")
}

fn activate(source: &mut SessionMachine, sink: &mut SessionMachine, binding: &SessionBinding) {
    let hello = binding.hello_frame();
    source.admit_outbound(hello).unwrap();
    sink.admit_inbound(hello).unwrap();
    sink.admit_outbound(hello).unwrap();
    source.admit_inbound(hello).unwrap();
    let ready = binding.frame(SessionMessage::Ready);
    source.admit_outbound(ready).unwrap();
    sink.admit_inbound(ready).unwrap();
    sink.admit_outbound(ready).unwrap();
    source.admit_inbound(ready).unwrap();
    assert!(source.is_active());
    assert!(sink.is_active());
}

fn receipt(
    source: &mut SessionMachine,
    sink: &mut SessionMachine,
    binding: &SessionBinding,
    message: SessionMessage<'_>,
) {
    let frame = binding.frame(message);
    sink.admit_outbound(frame).unwrap();
    source.admit_inbound(frame).unwrap();
}

fn close(source: &mut SessionMachine, sink: &mut SessionMachine, binding: &SessionBinding) {
    let final_sequence = TEXT_LAB_MAXIMUM_VALUES as u64;
    let closed = binding.frame(SessionMessage::InputClosed { final_sequence });
    source.admit_outbound(closed).unwrap();
    sink.admit_inbound(closed).unwrap();
    let terminal = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Completed,
        final_sequence,
    });
    source.admit_outbound(terminal).unwrap();
    sink.admit_inbound(terminal).unwrap();
    sink.admit_outbound(terminal).unwrap();
    source.admit_inbound(terminal).unwrap();
    assert!(source.is_terminal());
    assert!(sink.is_terminal());
}

#[test]
fn exact_two_line_sessions_carry_both_production_kernel_fragments_to_terminal() {
    let base = "ws://127.0.0.1:1/conduit";
    let exact = exact_text_lab_split_plan(
        base,
        &conduit_browser_runtime::presentation_nucleus::browser_text_upper_offer(),
    )
    .unwrap();
    let forward = binding(
        &exact.plan,
        TEXT_LAB_NATIVE_HOST,
        TEXT_LAB_BROWSER_HOST,
        TEXT_LAB_FORWARD_LINE,
    );
    let returned = binding(
        &exact.plan,
        TEXT_LAB_BROWSER_HOST,
        TEXT_LAB_NATIVE_HOST,
        TEXT_LAB_RETURN_LINE,
    );
    assert_ne!(forward.connection_id, returned.connection_id);
    assert_ne!(forward.attachment.line_id, returned.attachment.line_id);

    let mut forward_source = SessionMachine::new(forward.clone(), SessionRole::Source).unwrap();
    let mut forward_sink = SessionMachine::new(forward.clone(), SessionRole::Sink).unwrap();
    let mut return_source = SessionMachine::new(returned.clone(), SessionRole::Source).unwrap();
    let mut return_sink = SessionMachine::new(returned.clone(), SessionRole::Sink).unwrap();
    activate(&mut forward_source, &mut forward_sink, &forward);
    activate(&mut return_source, &mut return_sink, &returned);

    let mut native = NativeTextLabFragment::prepare(base).unwrap();
    let mut browser = BrowserTextLabFragment::prepare(base).unwrap();
    for expected in ["h", "e", "l", "l", "o"] {
        let offered = native.next_text_offer().unwrap();
        let frame = forward.frame(SessionMessage::Offered {
            sequence: offered.sequence,
            payload: &offered.bytes,
        });
        forward_source.admit_outbound(frame).unwrap();
        forward_sink.admit_inbound(frame).unwrap();
        browser
            .admit_text(offered.sequence, &offered.bytes)
            .unwrap();
        receipt(
            &mut forward_source,
            &mut forward_sink,
            &forward,
            SessionMessage::Accepted {
                sequence: offered.sequence,
            },
        );
        native.accept_text(offered.sequence).unwrap();
        receipt(
            &mut forward_source,
            &mut forward_sink,
            &forward,
            SessionMessage::Delivered {
                sequence: offered.sequence,
            },
        );
        native.deliver_text(offered.sequence).unwrap();

        let upper = browser.next_upper_offer().unwrap();
        assert_eq!(upper.sequence, offered.sequence);
        assert_eq!(upper.bytes, expected.to_ascii_uppercase().as_bytes());
        let frame = returned.frame(SessionMessage::Offered {
            sequence: upper.sequence,
            payload: &upper.bytes,
        });
        return_source.admit_outbound(frame).unwrap();
        return_sink.admit_inbound(frame).unwrap();
        native.admit_returned(upper.sequence, &upper.bytes).unwrap();
        receipt(
            &mut return_source,
            &mut return_sink,
            &returned,
            SessionMessage::Accepted {
                sequence: upper.sequence,
            },
        );
        browser.accept_upper(upper.sequence).unwrap();
        receipt(
            &mut return_source,
            &mut return_sink,
            &returned,
            SessionMessage::Delivered {
                sequence: upper.sequence,
            },
        );
        browser.deliver_upper(upper.sequence).unwrap();
        native
            .drive_presentation((upper.sequence + 1) as usize)
            .unwrap();
    }
    assert_eq!(native.presented(), "HELLO");

    close(&mut forward_source, &mut forward_sink, &forward);
    browser.close_text_input().unwrap();
    browser.finish().unwrap();
    close(&mut return_source, &mut return_sink, &returned);
    native.close_return_input().unwrap();
    native.finish().unwrap();
}
