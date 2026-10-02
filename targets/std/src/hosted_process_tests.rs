use super::*;
#[cfg(unix)]
fn shell(
    script: &str,
    stdin: &[u8],
    timeout: Duration,
    cancellation: impl Fn() -> bool,
) -> ProcessReport {
    let args = [OsString::from("-c"), OsString::from(script)];
    run_process(
        &ProcessRequest {
            program: Path::new("/bin/sh"),
            arguments: &args,
            environment: &[],
            stdin,
            maximum_stdout_bytes: 64,
            maximum_stderr_bytes: 32,
            timeout,
            require_process_group: true,
        },
        cancellation,
    )
    .unwrap()
}
#[test]
#[cfg(unix)]
fn writes_bounded_stdin_and_clears_environment() {
    let report = shell(
        "read line; printf '%s' \"$line\"; printf '%s' \"${HOME-unset}\" >&2",
        b"hello\n",
        Duration::from_secs(1),
        || false,
    );
    assert!(matches!(report.terminal, ProcessTerminal::Exited(status) if status.success()));
    assert_eq!(report.stdout.retained, b"hello");
    assert_eq!(report.stderr.retained, b"unset");
}
#[test]
#[cfg(unix)]
fn output_pressure_retains_bound_and_observes_full_stream() {
    let report = shell(
        "i=0; while [ $i -lt 1000 ]; do printf 12345678; i=$((i+1)); done",
        &[],
        Duration::from_secs(2),
        || false,
    );
    assert!(matches!(report.terminal, ProcessTerminal::Exited(status) if status.success()));
    assert_eq!(report.stdout.retained.len(), 64);
    assert_eq!(report.stdout.observed_bytes, 8000);
}
#[test]
#[cfg(unix)]
fn timeout_and_cancellation_retire_descendant_held_pipes() {
    for cancel in [false, true] {
        let started = Instant::now();
        let report = shell("sleep 30 & wait", &[], Duration::from_millis(80), || {
            cancel && started.elapsed() >= Duration::from_millis(20)
        });
        assert!(if cancel {
            matches!(report.terminal, ProcessTerminal::Cancelled)
        } else {
            matches!(report.terminal, ProcessTerminal::TimedOut)
        });
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
#[test]
#[cfg(unix)]
fn successful_parent_cannot_leave_pipe_readers_hanging() {
    let started = Instant::now();
    let report = shell(
        "sleep 30 & printf done",
        &[],
        Duration::from_secs(1),
        || false,
    );
    assert!(matches!(report.terminal, ProcessTerminal::Exited(status) if status.success()));
    assert_eq!(report.stdout.retained, b"done");
    assert!(started.elapsed() < Duration::from_secs(2));
}
#[test]
fn oversized_stdin_refuses_before_spawn() {
    let result = run_process(
        &ProcessRequest {
            program: Path::new("/not-executed"),
            arguments: &[],
            environment: &[],
            stdin: &[0; 257],
            maximum_stdout_bytes: 64,
            maximum_stderr_bytes: 32,
            timeout: Duration::from_secs(1),
            require_process_group: false,
        },
        || false,
    );
    assert!(matches!(result, Err(ProcessError::InvalidRequest(_))));
}

#[test]
#[cfg(unix)]
fn endless_output_cannot_starve_cancellation() {
    let started = Instant::now();
    let report = shell(
        "while :; do printf 1234567890123456789012345678901234567890; done",
        &[],
        Duration::from_secs(1),
        || started.elapsed() >= Duration::from_millis(40),
    );
    assert!(matches!(report.terminal, ProcessTerminal::Cancelled));
    assert_eq!(report.stdout.retained.len(), 64);
    assert!(report.stdout.observed_bytes > 64);
    assert_eq!(report.stdout.retained.capacity(), 64);
    assert!(started.elapsed() < Duration::from_secs(2));
}
#[test]
#[cfg(unix)]
fn prelaunch_cancellation_does_not_start_a_process() {
    let report = shell("exit 99", &[], Duration::from_secs(1), || true);
    assert!(matches!(report.terminal, ProcessTerminal::Cancelled));
    assert!(!report.launched);
    assert!(report.stdout.retained.is_empty());
}

#[test]
fn retirement_pipe_failure_preserves_the_established_terminal_cause() {
    for mut terminal in [
        Some(ProcessTerminal::Cancelled),
        Some(ProcessTerminal::TimedOut),
        Some(ProcessTerminal::ProviderLost(
            "original provider failure".into(),
        )),
    ] {
        let before = format!("{terminal:?}");
        record_pipe_failure(&mut terminal, io::Error::from(io::ErrorKind::BrokenPipe));
        assert_eq!(format!("{terminal:?}"), before);
    }
    let mut terminal = None;
    record_pipe_failure(&mut terminal, io::Error::from(io::ErrorKind::BrokenPipe));
    assert!(matches!(terminal, Some(ProcessTerminal::ProviderLost(_))));
}

#[test]
#[cfg(unix)]
fn pipe_failure_invalidates_apparent_success() {
    use std::os::unix::process::ExitStatusExt;
    let mut terminal = Some(ProcessTerminal::Exited(ExitStatus::from_raw(0)));
    record_pipe_failure(&mut terminal, io::Error::from(io::ErrorKind::BrokenPipe));
    assert!(matches!(terminal, Some(ProcessTerminal::ProviderLost(_))));
}
