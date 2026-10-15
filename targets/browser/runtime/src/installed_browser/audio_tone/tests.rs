
use super::*;

#[test]
fn admitted_renderer_keeps_phase_and_frame_continuity_in_hosted_pcm_storage() {
    let mut renderer = PreparedAudioTone {
        increment: None,
        phase: 0,
        start_frame: 0,
    };
    renderer
        .update(&Quantity::new(440, Unit::Hertz).encode())
        .unwrap();
    let first = renderer.render().unwrap();
    let second = renderer.render().unwrap();
    let (first_header, first_payload) = PcmFrameHeader::decode_frame(&first).unwrap();
    let (second_header, second_payload) = PcmFrameHeader::decode_frame(&second).unwrap();

    assert_eq!(first_header.frame_count, FRAMES);
    assert_eq!(first_header.start_frame, 0);
    assert_eq!(second_header.start_frame, u64::from(FRAMES));
    assert_eq!(first_payload.len(), usize::from(FRAMES) * 2);
    assert_eq!(second_payload.len(), usize::from(FRAMES) * 2);
    assert_ne!(first_payload, second_payload);
}
#[test]
fn theremin_renderer_accepts_precise_controller_pitch_at_the_phase_projection() {
    let frequency = Quantity::parse_plot_literal("1556.29595Hz").unwrap();
    let expected = ((155_629_595u128 * (1u128 << 32)) / (100_000 * u128::from(SAMPLE_RATE))) as u32;
    assert_eq!(phase_increment(frequency), Some(expected));
    let mut renderer = PreparedAudioTone {
        increment: None,
        phase: 0,
        start_frame: 0,
    };
    renderer.update(&frequency.encode()).unwrap();
    renderer.render().unwrap();
    renderer.render().unwrap();
    assert_eq!(renderer.start_frame, 2 * u64::from(FRAMES));
    assert!(phase_increment(Quantity::new(19, Unit::Hertz)).is_none());
    assert!(phase_increment(Quantity::new(20_001, Unit::Hertz)).is_none());
}

#[test]
fn one_current_frequency_update_drives_multiple_independent_cadence_blocks() {
    use conduit_kernel::HostCallOutcome;
    fn value(slot: u16, length: usize) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: length as u32,
        }
    }
    let mut back = AudioToneBack {
        empty: value(0, 0),
        next_request: 0,
        pending: None,
        frequency_ready: false,
        closed: false,
    };
    let frequency = Quantity::new(440, Unit::Hertz).encode();
    let tick = conduit_time::encode_tick(0);
    let mut update = StepIo::test_frame(
        [
            Some(value(1, frequency.len())),
            Some(value(2, tick.len())),
            None,
        ],
        [false; 3],
        [Some(BLOCK_BYTES as u32), None, None],
        None,
        8,
    );
    assert_eq!(
        back.step(
            &mut update,
            &StepInputBytes::test_frame([Some(&frequency), Some(&tick), None], None)
        ),
        StepOutcome::Progress
    );
    assert!(update.test_consumed(PortId(0)));
    assert!(
        !update.test_consumed(PortId(1)),
        "frequency replacement wins a simultaneous cadence tick"
    );
    assert_eq!(
        update
            .test_host_request()
            .map(|request| (request.0, request.1)),
        Some((RequestId(0), HostCallId(0)))
    );
    let mut renderer = PreparedAudioTone {
        increment: None,
        phase: 0,
        start_frame: 0,
    };
    renderer.update(&frequency).unwrap();
    let completed = HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: None,
        failure: None,
    };
    let mut completion = StepIo::test_frame(
        [None; 3],
        [false; 3],
        [Some(BLOCK_BYTES as u32), None, None],
        Some((RequestId(0), completed)),
        8,
    );
    assert_eq!(
        back.step(
            &mut completion,
            &StepInputBytes::test_frame([None; 3], None)
        ),
        StepOutcome::Progress
    );
    for index in 0..3u32 {
        let tick = conduit_time::encode_tick(u64::from(index));
        let mut cadence = StepIo::test_frame(
            [None, Some(value(2, tick.len())), None],
            [false; 3],
            [Some(BLOCK_BYTES as u32), None, None],
            None,
            8,
        );
        assert_eq!(
            back.step(
                &mut cadence,
                &StepInputBytes::test_frame([None, Some(&tick), None], None)
            ),
            StepOutcome::Progress
        );
        assert!(cadence.test_consumed(PortId(1)));
        let request = RequestId(index + 1);
        assert_eq!(
            cadence
                .test_host_request()
                .map(|request| (request.0, request.1)),
            Some((request, HostCallId(1)))
        );
        let block = renderer.render().unwrap();
        let (header, _) = PcmFrameHeader::decode_frame(&block).unwrap();
        assert_eq!(header.start_frame, u64::from(index) * u64::from(FRAMES));
        let frame = value(3, block.len());
        let outcome = HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(frame, BLOCK_BYTES as u32).unwrap()),
            failure: None,
        };
        let mut completion = StepIo::test_frame(
            [None; 3],
            [false; 3],
            [Some(BLOCK_BYTES as u32), None, None],
            Some((request, outcome)),
            8,
        );
        assert_eq!(
            back.step(
                &mut completion,
                &StepInputBytes::test_frame([None; 3], None)
            ),
            StepOutcome::Progress
        );
        assert_eq!(completion.test_output(PortId(0)), Some(frame));
    }
    assert_eq!(
        back.next_request, 4,
        "one update plus three renders; no repeated sampling update"
    );
}
