#![cfg(feature = "kernel")]
#[path = "common/kernel_graph.rs"]
mod graph;
use conduit_kernel::{
    scheduler::{SchedulerError, StepBack, StepInputBytes, StepIo, StepOutcome},
    FailureCode, KernelEventKind, PortId, SignQuery, ValueRef,
};
use conduit_speech::{
    kernel::{NativeSpeechBack, PreparationRefusal, REQUIRED_STEP_FUEL},
    pronounce, Renderer, VoiceBoundary, VoiceEvent, MAXIMUM_EVENTS,
};
use std::{cell::Cell, rc::Rc};

fn voice() -> NativeSpeechBack {
    let plan = graph::plan();
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == conduit_speech::kernel::KIND)
        .unwrap();
    NativeSpeechBack::prepare::<1>(gear, PortId(0), PortId(0)).unwrap()
}
fn reference(text: &[u8]) -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 7,
        byte_len: text.len() as u32,
    }
}
#[test]
fn proposal_pressure_and_rejected_transaction_preserve_the_cursor() {
    let text = b"Hello world";
    let reference = reference(text);
    let mut back = voice();
    let bytes = StepInputBytes::test_frame([Some(text.as_slice())], None);
    let mut blocked =
        StepIo::test_frame([Some(reference)], [false], [None], None, REQUIRED_STEP_FUEL);
    assert_eq!(back.step(&mut blocked, &bytes), StepOutcome::Await);
    assert_eq!(back.rendered_frames(), 0);
    let mut ready = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(285)],
        None,
        REQUIRED_STEP_FUEL,
    );
    assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Progress);
    assert!(
        !ready.test_consumed(PortId(0)),
        "the exact source stays retained while speech is active"
    );
    let proposed = StepBack::<1>::prepared_output(&back, PortId(0))
        .unwrap()
        .to_vec();
    assert_eq!(back.rendered_frames(), 0);
    // No commit: model rejection by another atomic participant.
    let mut retry = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(285)],
        None,
        REQUIRED_STEP_FUEL,
    );
    assert_eq!(back.step(&mut retry, &bytes), StepOutcome::Progress);
    assert_eq!(
        StepBack::<1>::prepared_output(&back, PortId(0)),
        Some(proposed.as_slice())
    );
    StepBack::<1>::step_committed(&mut back);
    assert_eq!(back.rendered_frames(), 128);
    let mut blocked =
        StepIo::test_frame([Some(reference)], [false], [None], None, REQUIRED_STEP_FUEL);
    assert_eq!(back.step(&mut blocked, &bytes), StepOutcome::Await);
    assert_eq!(back.rendered_frames(), 128);
    let mut exhausted = StepIo::test_frame([Some(reference)], [false], [Some(285)], None, 0);
    assert!(
        matches!(back.step(&mut exhausted, &bytes), StepOutcome::Fail(f) if f.code == FailureCode::WorkBudgetExhausted)
    );
    assert_eq!(back.rendered_frames(), 128);
}
#[test]
fn preparation_rejects_fore_artifact_pool_and_configuration_drift() {
    let plan = graph::plan();
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == conduit_speech::kernel::KIND)
        .unwrap();
    for change in 0..4 {
        let mut wrong = gear.clone();
        match change {
            0 => wrong.outputs[0].value_kind = conduit_core::kind_id("value/text"),
            1 => wrong.artifact_id = conduit_core::ArtifactId::from("foreign"),
            2 => wrong.configuration[0].value = conduit_core::ConfigurationValue::U64(0),
            _ => wrong
                .pool_references
                .push(conduit_core::SharedPoolId::from("ambient")),
        }
        assert!(matches!(
            NativeSpeechBack::prepare::<1>(&wrong, PortId(0), PortId(0)),
            Err(PreparationRefusal::Identity | PreparationRefusal::Configuration)
        ));
    }
    assert!(matches!(
        NativeSpeechBack::prepare::<1>(gear, PortId(1), PortId(0)),
        Err(PreparationRefusal::Port)
    ));
}
#[test]
fn planned_kernel_fanout_preserves_every_frame_and_drains_before_completion() {
    let pause = Rc::new(Cell::new(true));
    let mut scheduler = graph::scheduler(b"Hello world", pause.clone());
    for _ in 0..32 {
        scheduler.step().unwrap();
    }
    let before = scheduler
        .drivers()
        .iter()
        .find_map(|d| match d {
            graph::Driver::Voice(b) => Some(b.rendered_frames()),
            _ => None,
        })
        .unwrap();
    assert_eq!(before, 128, "one block may enter each admitted branch");
    for _ in 0..32 {
        scheduler.step().unwrap();
    }
    let after = scheduler
        .drivers()
        .iter()
        .find_map(|d| match d {
            graph::Driver::Voice(b) => Some(b.rendered_frames()),
            _ => None,
        })
        .unwrap();
    assert_eq!(after, before, "a full branch pressures the atomic fan-out");
    pause.set(false);
    scheduler.run(20000).unwrap();
    let mut events = [VoiceEvent::boundary(VoiceBoundary::phrase); MAXIMUM_EVENTS];
    let prepared = pronounce("Hello world", &mut events).unwrap();
    let mut renderer = Renderer::prepare(prepared.events()).unwrap();
    let mut expected = Vec::new();
    while !renderer.is_complete() {
        let mut block = [0_i16; 128];
        let n = renderer.render(&mut block).unwrap();
        for sample in &block[..n] {
            expected.extend_from_slice(&sample.to_le_bytes());
        }
    }
    let mut sinks = 0;
    for driver in scheduler.drivers() {
        if let graph::Driver::Sink { pcm, complete, .. } = driver {
            assert!(*complete);
            assert_eq!(*pcm, expected);
            sinks += 1;
        }
    }
    assert_eq!(sinks, 2);
    assert!(scheduler
        .signs()
        .contains_kind(KernelEventKind::BackCompleted));
}
#[test]
fn cancellation_invalid_text_and_empty_utterance_are_distinct() {
    let mut cancelled = graph::scheduler(b"Hello world", Rc::new(Cell::new(true)));
    for _ in 0..20 {
        cancelled.step().unwrap();
    }
    cancelled.cancel().unwrap();
    assert!(cancelled
        .signs()
        .contains_kind(KernelEventKind::RunCancelled));
    assert_eq!(cancelled.run(20000), Err(SchedulerError::Cancelled));
    for (text, detail) in [
        (
            b"\xff".as_slice(),
            conduit_speech::kernel::InputFailureDetail::InvalidUtf8,
        ),
        (
            b"123".as_slice(),
            conduit_speech::kernel::InputFailureDetail::UnsupportedCharacter,
        ),
    ] {
        let mut invalid = graph::scheduler(text, Rc::new(Cell::new(false)));
        assert!(
            matches!(invalid.run(20000), Err(SchedulerError::BackFailed(f)) if f.code == FailureCode::InvalidInput && f.detail == detail as u16)
        );
    }
    let mut empty = graph::scheduler(b"", Rc::new(Cell::new(false)));
    empty.run(20000).unwrap();
    for driver in empty.drivers() {
        if let graph::Driver::Sink { pcm, complete, .. } = driver {
            assert!(pcm.is_empty());
            assert!(*complete);
        }
    }
}
