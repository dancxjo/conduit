//! A provider's exited parent does not imply EOF on inherited pipe writers.
use super::*;

fn inherited_writer_capture(cancel: bool) {
    let _process = microphone_process();
    let (root, discovery) = parsed_fixture(
        "#!/bin/sh\nsleep 3 &\necho $! > \"$0.writer\"\nexit 0\n",
        if cancel {
            "inherited-cancel"
        } else {
            "inherited-timeout"
        },
    );
    let selected = discovery.observations[0].clone();
    let mut adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: if cancel {
                    Duration::from_secs(2)
                } else {
                    Duration::from_millis(100)
                },
            },
        )
        .unwrap();
    let started = Instant::now();
    let result = adapter.capture(|| cancel && started.elapsed() >= Duration::from_millis(100));
    let elapsed = started.elapsed();
    // Retire the deliberately inherited writer even when the assertion fails.
    let pid = fs::read_to_string(root.join("arecord.writer")).unwrap();
    let status = Command::new("kill").arg(pid.trim()).status().unwrap();
    assert!(status.success());
    assert_eq!(
        result,
        Err(if cancel {
            MicrophoneFailure::Cancelled
        } else {
            MicrophoneFailure::Timeout
        })
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "inherited pipe held capture for {elapsed:?}"
    );
    assert!(adapter.take_receipt().is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn timeout_covers_pipe_drain_after_provider_exit() {
    inherited_writer_capture(false);
}
#[test]
fn cancellation_covers_pipe_drain_after_provider_exit() {
    inherited_writer_capture(true);
}

#[test]
fn noisy_pipe_refuses_at_the_byte_bound_before_timeout() {
    let _process = microphone_process();
    let (root, discovery) = parsed_fixture(
        "#!/bin/sh\nwhile :; do printf '0123456789012345678901234567890123456789'; done\n",
        "pipe-overflow",
    );
    let selected = discovery.observations[0].clone();
    let mut adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    assert_eq!(
        adapter.capture(|| false),
        Err(MicrophoneFailure::OutputOverflow)
    );
    assert!(adapter.take_receipt().is_none());
    fs::remove_dir_all(root).unwrap();
}
