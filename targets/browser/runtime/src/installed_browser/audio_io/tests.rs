use super::*;
use conduit_kernel::HostCallOutcome;

fn value(slot: u16, byte_len: usize) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: byte_len as u32,
    }
}

#[test]
fn browser_pcm_offers_seal_distinct_resource_and_authority_truth() {
    let capture = capture_offer();
    assert_eq!(
        capture.kind_id.as_str(),
        conduit_semantic_catalog::AUDIO_CAPTURE_PUSH_TO_TALK_KIND
    );
    assert_eq!(
        capture.host_calls[0].contract_id.as_str(),
        CAPTURE_OPERATION
    );
    assert_eq!(
        capture.resource_requirements[0].class_id.as_str(),
        CAPTURE_RESOURCE
    );
    assert_eq!(
        capture.authority_requirements[0].contract_id.as_str(),
        CAPTURE_AUTHORITY
    );
    assert_eq!(
        capture.host_calls[0].target_kind,
        Some(capture.authority_requirements[0].subject_kind.clone())
    );
    assert_eq!(
        capture.outputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );

    let playback = playback_offer();
    assert_eq!(
        playback.kind_id.as_str(),
        conduit_semantic_catalog::AUDIO_PLAY_KIND
    );
    assert_eq!(playback.host_calls[0].contract_id.as_str(), PLAY_OPERATION);
    assert_eq!(
        playback.resource_requirements[0].class_id.as_str(),
        PLAY_RESOURCE
    );
    assert_eq!(
        playback.authority_requirements[0].contract_id.as_str(),
        PLAY_AUTHORITY
    );
}

#[test]
fn capture_preserves_pending_frame_under_output_pressure_and_rearms() {
    let mut operation = CaptureBack {
        request: value(0, 1),
        next: 0,
        pending: false,
        completed: false,
    };
    let mut start = StepIo::test_frame([None], [false], [Some(64)], None, 4);
    assert_eq!(
        operation.step(&mut start, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(
        start.test_host_request().map(|request| request.0),
        Some(RequestId(0))
    );
    let frame = value(1, 32);
    let outcome = HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: Some(BoundedValueRef::new(frame, 64).unwrap()),
        failure: None,
    };
    let mut blocked = StepIo::test_frame([None], [false], [None], Some((RequestId(0), outcome)), 5);
    assert_eq!(
        operation.step(&mut blocked, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Await
    );
    assert!(operation.pending);
    assert_eq!(operation.next, 0);
    let mut ready = StepIo::test_frame(
        [None],
        [false],
        [Some(64)],
        Some((RequestId(0), outcome)),
        5,
    );
    assert_eq!(
        operation.step(&mut ready, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(ready.test_output(PortId(0)), Some(frame));
    assert_eq!(
        ready.test_host_request().map(|request| request.0),
        Some(RequestId(1))
    );
}

#[test]
fn playback_validates_pcm_before_request_and_completes_after_closure() {
    let payload = [0_u8; 4];
    let frame = conduit_audio::PcmFrameHeader::new(
        conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
        16_000,
        conduit_audio::PcmChannelLayout::Mono,
        2,
        1,
        0,
        false,
    )
    .unwrap()
    .encode_frame(&payload)
    .unwrap();
    let input = value(1, frame.len());
    let mut operation = PlaybackBack {
        budget: PlaybackBudget::new(3072, 16384).unwrap(),
        next: 0,
        pending: None,
    };
    let mut io = StepIo::test_frame([Some(input)], [false], [None], None, 4);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([Some(&frame)], None),),
        StepOutcome::Progress
    );
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(0))
    );
    let completion = HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: None,
        failure: None,
    };
    let mut completed =
        StepIo::test_frame([None], [false], [None], Some((RequestId(0), completion)), 4);
    assert_eq!(
        operation.step(&mut completed, &StepInputBytes::test_frame([None], None),),
        StepOutcome::Progress
    );
    let mut closed = StepIo::test_frame([None], [true], [None], None, 4);
    assert_eq!(
        operation.step(&mut closed, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
}

#[test]
fn playback_refuses_excess_work_before_requesting_a_platform_effect() {
    let frame = conduit_audio::PcmFrameHeader::new(
        conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
        8000,
        conduit_audio::PcmChannelLayout::Mono,
        9,
        1,
        9_000_000,
        false,
    )
    .unwrap()
    .encode_frame(&[0; 18])
    .unwrap();
    let mut operation = PlaybackBack {
        budget: PlaybackBudget::new(1, 1).unwrap(),
        next: 0,
        pending: None,
    };
    let mut io = StepIo::test_frame([Some(value(1, frame.len()))], [false], [None], None, 4);
    assert!(matches!(
        operation.step(&mut io, &StepInputBytes::test_frame([Some(&frame)], None)),
        StepOutcome::Fail(Failure {
            code: FailureCode::WorkBudgetExhausted,
            ..
        })
    ));
    assert!(io.test_host_request().is_none());
    assert!(operation.pending.is_none());
    assert_eq!(operation.next, 0);
}
