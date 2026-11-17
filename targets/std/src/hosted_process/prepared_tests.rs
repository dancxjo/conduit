//! Qualified producer and effect-boundary checks for the same process loop.
use super::*;
#[test]
#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn funded_process_positive_qualification_and_requested_allocations_fit_recipe() {
    assert!(
        std::hint::black_box(cfg!(conduit_prepared_process_layout)),
        "{}",
        env!("CONDUIT_PREPARED_PROCESS_LAYOUT")
    );
    let arguments: Vec<OsString> = (0..14).map(|_| OsString::from("argument")).collect();
    let environment = [("LC_ALL".into(), "C".into())];
    let request = ProcessRequest {
        program: Path::new("/usr/bin/true"),
        arguments: &arguments,
        environment: &environment,
        stdin: &[],
        maximum_stdout_bytes: 64,
        maximum_stderr_bytes: 64,
        timeout: Duration::from_secs(2),
        require_process_group: true,
    };
    let layout = layout::process_layout(
        request.program.as_os_str().as_encoded_bytes().len(),
        arguments.iter().map(|arg| arg.len()),
        64,
        64,
    )
    .unwrap();
    let probe = crate::allocation_probe::begin();
    let prepared = PreparedProcess::prepare_funded(&request, 0);
    let preparation = probe.finish_bytes();
    let prepared = prepared.unwrap();
    assert!(
        preparation <= layout.preparation_bytes,
        "{preparation} > {}",
        layout.preparation_bytes
    );
    let probe = crate::allocation_probe::begin();
    let result = prepared.run(&[], || false);
    let execution = probe.finish_bytes();
    let result = result.unwrap();
    assert!(matches!(result.terminal,ProcessTerminal::Exited(status) if status.success()));
    assert!(
        execution <= layout.spawn_bytes,
        "{execution} > {}",
        layout.spawn_bytes
    );
}
#[test]
#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn funded_cancel_and_launch_failure_retire_without_hidden_growth() {
    let environment = [("LC_ALL".into(), "C".into())];
    let request = ProcessRequest {
        program: Path::new("/nonexistent/conduit-provider"),
        arguments: &[],
        environment: &environment,
        stdin: &[],
        maximum_stdout_bytes: 64,
        maximum_stderr_bytes: 64,
        timeout: Duration::from_secs(2),
        require_process_group: true,
    };
    let cancelled = PreparedProcess::prepare_funded(&request, 0).unwrap();
    let probe = crate::allocation_probe::begin();
    let report = cancelled.run(&[], || true);
    let bytes = probe.finish_bytes();
    assert!(matches!(
        report.unwrap().terminal,
        ProcessTerminal::Cancelled
    ));
    assert_eq!(bytes, 0);
    let prepared = PreparedProcess::prepare_funded(&request, 0).unwrap();
    let layout = layout::process_layout(
        request.program.as_os_str().as_encoded_bytes().len(),
        [],
        64,
        64,
    )
    .unwrap();
    let probe = crate::allocation_probe::begin();
    let result = prepared.run(&[], || false);
    let bytes = probe.finish_bytes();
    assert!(matches!(result, Err(ProcessError::Launch(_))));
    assert!(
        bytes <= layout.spawn_bytes,
        "{bytes} > {}",
        layout.spawn_bytes
    );
}

#[test]
#[cfg(not(all(
    target_os = "linux",
    target_env = "gnu",
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
fn unsupported_platform_has_no_funded_process_receipt() {
    assert!(layout::process_layout(14, [], 0, 4096).is_err());
    let environment = [("LC_ALL".into(), "C".into())];
    let request = ProcessRequest {
        program: Path::new("/usr/bin/aplay"),
        arguments: &[],
        environment: &environment,
        stdin: &[],
        maximum_stdout_bytes: 0,
        maximum_stderr_bytes: 4096,
        timeout: Duration::from_secs(2),
        require_process_group: true,
    };
    assert!(matches!(
        PreparedProcess::prepare_funded(&request, 0),
        Err(ProcessError::Unsupported(_))
    ));
}

#[test]
#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn malformed_nul_refuses_before_command_allocation_or_a_prepared_child_exists() {
    let environment = [("LC_ALL".into(), "C".into())];
    let arguments = [OsString::from("\0")];
    for (program, arguments) in [
        (Path::new("/usr/bin/true"), arguments.as_slice()),
        (Path::new("/invalid\0program"), &[][..]),
    ] {
        let request = ProcessRequest {
            program,
            arguments,
            environment: &environment,
            stdin: &[],
            maximum_stdout_bytes: 0,
            maximum_stderr_bytes: 0,
            timeout: Duration::from_secs(2),
            require_process_group: true,
        };
        let probe = crate::allocation_probe::begin();
        let result = PreparedProcess::prepare_funded(&request, 0);
        let requested = probe.finish_bytes();
        assert!(matches!(result, Err(ProcessError::InvalidRequest(_))));
        assert_eq!(
            requested, 0,
            "malformed path must refuse before std's alternate CString producer"
        );
    }
}
