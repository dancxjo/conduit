//! Finite provider experiments, distinct from an admitted Plan or physical mic.
use super::super::flow_provider::{MicrophoneFlow, MicrophoneFlowPoll};
use super::*;
fn next(flow: &mut MicrophoneFlow) -> Result<MicrophoneFlowPoll, MicrophoneFailure> {
    loop {
        let outcome = flow.poll(false)?;
        if outcome != MicrophoneFlowPoll::Pending {
            return Ok(outcome);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn prepared(script: &str, name: &str, milliseconds: u32, block: u16) -> (PathBuf, MicrophoneFlow) {
    let prelude = "printf 'stream: CAPTURE\naccess: RW_INTERLEAVED\nformat: S16_LE\nchannels: 1\nrate: 16000\nexact rate: 16000 (16000/1)\nbuffer_size: 1024\nperiod_size: 256\nRecording raw data stdout : Signed 16 bit Little Endian, Rate 16000 Hz, Mono\n' >&2\n";
    let script = script.replacen("#!/bin/sh\n", &format!("#!/bin/sh\n{prelude}"), 1);
    let (root, discovery) = parsed_fixture(&script, name);
    let adapter = discovery
        .clone()
        .initialize(
            &discovery.observations[0],
            MicrophoneLimits {
                capture_milliseconds: milliseconds,
                timeout: Duration::from_millis(500),
            },
        )
        .unwrap();
    let flow = adapter.prepare_pcm_flow(block).unwrap();
    (root, flow.start(false).unwrap())
}
#[test]
fn flow_emits_before_provider_completion_and_cancel_retires_group() {
    let _process = microphone_process();
    let (root, mut flow) = prepared(
        "#!/bin/sh\nprintf '\\001\\002\\003\\004'\nsleep 10\n",
        "live-prefix",
        1,
        2,
    );
    let started = Instant::now();
    assert_eq!(next(&mut flow), Ok(MicrophoneFlowPoll::Frame));
    assert!(started.elapsed() < Duration::from_millis(400));
    let (header, bytes) =
        conduit_audio::PcmFrameHeader::decode_frame(flow.frame().unwrap()).unwrap();
    assert_eq!(header.start_frame(), 0);
    assert_eq!(bytes, [1, 2, 3, 4]);
    assert_eq!(flow.poll(true), Err(MicrophoneFailure::Cancelled));
    assert!(flow.frame().is_none());
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(flow);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn flow_pressure_is_terminal_instead_of_hidden_drop_or_growth() {
    let _process = microphone_process();
    let (root, mut flow) = prepared(
        "#!/bin/sh\nprintf '\\001\\002\\003\\004'\nsleep 10\n",
        "pressure",
        1,
        2,
    );
    assert_eq!(next(&mut flow), Ok(MicrophoneFlowPoll::Frame));
    assert_eq!(flow.poll(false), Err(MicrophoneFailure::CaptureOverrun));
    assert!(flow.frame().is_none());
    drop(flow);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn flow_preserves_exact_short_tail_and_completes_only_after_eof() {
    let _process = microphone_process();
    // One millisecond is16frames; ten-frame blocks yield a six-frame tail.
    let (root, mut flow) = prepared(
        "#!/bin/sh\nprintf '01234567890123456789012345678901'\n",
        "tail",
        1,
        10,
    );
    let capacity = flow.storage_capacity();
    let mut starts = Vec::new();
    let mut samples = Vec::new();
    while next(&mut flow).unwrap() == MicrophoneFlowPoll::Frame {
        let (header, payload) =
            conduit_audio::PcmFrameHeader::decode_frame(flow.frame().unwrap()).unwrap();
        starts.push((
            header.start_frame(),
            header.frame_count(),
            header.clock_id(),
        ));
        samples.extend_from_slice(payload);
        flow.release_frame().unwrap();
        assert_eq!(flow.storage_capacity(), capacity);
    }
    assert_eq!(samples, b"01234567890123456789012345678901");
    assert_eq!((starts[0].0, starts[0].1), (0, 10));
    assert_eq!((starts[1].0, starts[1].1), (10, 6));
    assert_eq!(starts[0].2, starts[1].2);
    drop(flow);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn flow_partial_sample_overflow_and_provider_loss_remain_distinct() {
    let _process = microphone_process();
    for (name, script, expected) in [
        (
            "partial",
            "#!/bin/sh\nprintf 'abc'\n",
            MicrophoneFailure::PartialSampleFrame,
        ),
        (
            "overflow",
            "#!/bin/sh\nprintf '012345678901234567890123456789012'\n",
            MicrophoneFailure::OutputOverflow,
        ),
        (
            "loss",
            "#!/bin/sh\nexit 7\n",
            MicrophoneFailure::ProviderLost,
        ),
    ] {
        let (root, mut flow) = prepared(script, name, 1, 16);
        loop {
            match next(&mut flow) {
                Ok(MicrophoneFlowPoll::Frame) => flow.release_frame().unwrap(),
                Err(error) => {
                    assert_eq!(error, expected);
                    break;
                }
                other => panic!("unexpected outcome: {other:?}"),
            }
        }
        drop(flow);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn flow_deadline_covers_inherited_pipe_writer_after_parent_exit() {
    let _process = microphone_process();
    let (root, mut flow) = prepared(
        "#!/bin/sh\nprintf '01234567890123456789012345678901'\nsleep 10 &\nexit 0\n",
        "inherited",
        1,
        16,
    );
    let started = Instant::now();
    assert_eq!(next(&mut flow), Ok(MicrophoneFlowPoll::Frame));
    flow.release_frame().unwrap();
    assert_eq!(next(&mut flow), Err(MicrophoneFailure::Timeout));
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(flow);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn prepared_capture_clocks_are_distinct_and_polling_does_not_allocate() {
    let _process = microphone_process();
    let (root, mut flow) = prepared(
        "#!/bin/sh\nprintf '01234567890123456789012345678901'\n",
        "allocation-clock",
        1,
        10,
    );
    let (other_root, other) = prepared(
        "#!/bin/sh\nprintf '01234567890123456789012345678901'\n",
        "other-clock",
        1,
        10,
    );
    assert_ne!(flow.clock_id(), other.clock_id());
    drop(other);
    let guard = crate::allocation_probe::begin();
    let mut blocks = 0;
    loop {
        match next(&mut flow).unwrap() {
            MicrophoneFlowPoll::Frame => {
                conduit_audio::PcmFrameHeader::decode_frame(flow.frame().unwrap()).unwrap();
                flow.release_frame().unwrap();
                blocks += 1;
            }
            MicrophoneFlowPoll::Complete => break,
            MicrophoneFlowPoll::Pending => unreachable!(),
        }
    }
    let allocations = guard.finish();
    assert_eq!(blocks, 2);
    assert_eq!(allocations, 0);
    drop(flow);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(other_root).unwrap();
}
