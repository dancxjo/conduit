use super::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "../../../../../semantics/chat/tests/common_episode_retrieval/mod.rs"]
mod common;
fn context() -> Vec<u8> {
    let context:conduit_body::BodyConversationContext=serde_json::from_value(serde_json::json!({"schema":"conduit.body/conversation-context-value@2","display_name":"Pete","body_id":"body/one","wake_id":"wake/one","wake_sequence":3,"basis":{"body_id":"body/one","wake_id":"wake/one","wake_sequence":3,"revision":1},"hosts":[],"active_plots":[],"current_plan_id":null,"active_play_id":null,"lines":[],"recent_sign_ids":[]})).unwrap();
    conduit_chat::encode_body_conversation_context(&context).unwrap()
}
fn value(b: &[u8], slot: u16) -> conduit_kernel::ValueRef {
    conduit_kernel::ValueRef {
        slot,
        generation: 1,
        byte_len: b.len() as u32,
    }
}
fn prepared_back() -> EpisodeRetrievalProducerBack {
    let mut producer = PreparedEpisodeRetrievalProducer::prepare();
    producer.admit_context(&context()).unwrap();
    EpisodeRetrievalProducerBack {
        producer: Box::new(producer),
        staged: false,
        current: core::array::from_fn(|_| Vec::with_capacity(4096)),
        candidate: Vec::with_capacity(4096),
        candidate_slot: None,
        context_closed: false,
        experience_closed: false,
        complete: false,
    }
}
fn admit_currents(back: &mut EpisodeRetrievalProducerBack, f: &common::Fixture) {
    let clock = conduit_chat::EpisodeRetrievalClockClass::MonotonicMilliseconds
        .encode()
        .unwrap();
    for (p, data) in [
        (INTENT, f.intent.as_slice()),
        (POLICY, f.policy.as_slice()),
        (CLOCK, clock.as_slice()),
    ] {
        let mut inputs = [None; 6];
        inputs[p.0 as usize] = Some(data);
        let refs = core::array::from_fn(|i| inputs[i].map(|b| value(b, i as u16)));
        let mut io = StepIo::test_frame(refs, [false; 6], [None; 6], None, 16);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame(inputs, None)),
            StepOutcome::Progress
        );
        assert!(io.test_consumed(p));
        StepBack::<6>::step_committed(back);
    }
}
#[test]
fn window_capture_and_intent_commit_atomically_under_pressure() {
    let f = common::Fixture::new();
    let input = [
        Some(f.experience.as_slice()),
        Some(f.memory.as_slice()),
        None,
        None,
        None,
        None,
    ];
    let refs = core::array::from_fn(|i| input[i].map(|b| value(b, i as u16)));
    let bytes = StepInputBytes::test_frame(input, None);
    let mut back = prepared_back();
    admit_currents(&mut back, &f);
    let mut small = StepIo::test_frame(
        refs,
        [false; 6],
        [Some(65536), Some(1), None, None, None, None],
        None,
        16,
    );
    assert_eq!(back.step(&mut small, &bytes), StepOutcome::Await);
    for p in [EXPERIENCE, MEMORY] {
        assert!(!small.test_consumed(p));
    }
    assert!(StepBack::<6>::prepared_output(&back, CAPTURE).is_none());
    let mut ready = StepIo::test_frame(
        refs,
        [false; 6],
        [Some(65536), Some(MAX as u32), Some(4096), None, None, None],
        None,
        16,
    );
    assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Progress);
    for p in [EXPERIENCE, MEMORY] {
        assert!(ready.test_consumed(p));
    }
    assert_eq!(
        StepBack::<6>::prepared_output(&back, INTENT_CURRENT),
        Some(f.intent.as_slice())
    );
    let capture = StepBack::<6>::prepared_output(&back, CAPTURE)
        .unwrap()
        .to_vec();
    let window = StepBack::<6>::prepared_output(&back, WINDOW)
        .unwrap()
        .to_vec();
    let mut loader = conduit_chat::PreparedEpisodeRetrievalPacket::prepare();
    let (snapshot, _) = loader.decode(&capture).unwrap();
    let mut verify = conduit_chat::PreparedEpisodeRetrievalWindow::prepare();
    assert_eq!(verify.build(&snapshot).unwrap(), window);
    StepBack::<6>::step_committed(&mut back);
    assert!(StepBack::<6>::prepared_output(&back, CAPTURE).is_none());
    let mut duplicate = StepIo::test_frame(
        refs,
        [false; 6],
        [Some(65536), Some(MAX as u32), Some(4096), None, None, None],
        None,
        16,
    );
    assert!(matches!(
        back.step(&mut duplicate, &bytes),
        StepOutcome::Fail(_)
    ));
    assert!(!duplicate.test_consumed(EXPERIENCE));
}
#[test]
fn cancellation_revokes_staged_capture_and_terminal_memory_must_complete() {
    let mut back = prepared_back();
    let f = common::Fixture::new();
    let bytes = StepInputBytes::test_frame([None; 6], None);
    let mut close = StepIo::test_frame(
        [None; 6],
        [true, false, false, false, false, false],
        [None; 6],
        None,
        16,
    );
    assert_eq!(back.step(&mut close, &bytes), StepOutcome::Progress);
    let inputs = [None, Some(f.memory.as_slice()), None, None, None, None];
    let bytes = StepInputBytes::test_frame(inputs, None);
    let refs = core::array::from_fn(|i| inputs[i].map(|b| value(b, i as u16)));
    let mut finish = StepIo::test_frame(refs, [false; 6], [None; 6], None, 16);
    assert_eq!(back.step(&mut finish, &bytes), StepOutcome::Complete);
    assert!(finish.test_consumed(MEMORY));
    let mut back = prepared_back();
    StepBack::<6>::cancel(&mut back);
    let mut io = StepIo::test_frame(
        [None; 6],
        [false; 6],
        [Some(65536), Some(MAX as u32), Some(4096), None, None, None],
        None,
        16,
    );
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([None; 6], None)),
        StepOutcome::Complete
    );
    assert!(StepBack::<6>::prepared_output(&back, CAPTURE).is_none());
}

#[test]
fn one_current_profile_supports_two_windows_without_republication() {
    let f = common::Fixture::new();
    let mut back = prepared_back();
    admit_currents(&mut back, &f);
    let advanced = common::memory(&[1, 2, 3], "body/one", "wake/one");
    for memory in [f.memory.as_slice(), advanced.as_slice()] {
        let inputs = [
            Some(f.experience.as_slice()),
            Some(memory),
            None,
            None,
            None,
            None,
        ];
        let refs = core::array::from_fn(|i| inputs[i].map(|b| value(b, i as u16)));
        let mut io = StepIo::test_frame(
            refs,
            [false; 6],
            [Some(65536), Some(MAX as u32), Some(4096), None, None, None],
            None,
            16,
        );
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame(inputs, None)),
            StepOutcome::Progress
        );
        assert!(StepBack::<6>::prepared_output(&back, CAPTURE).is_some());
        StepBack::<6>::step_committed(&mut back);
    }
}
