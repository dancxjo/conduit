#![cfg(target_os = "linux")]
use conduit_plot::*;
use conduit_std_host::source_check::process::SourceCheckProcessInput;
use std::{
    io::Write,
    process::{Command, Output, Stdio},
};
fn input(source: &str) -> (SourceCheckProcessInput, SourceCheckProjection) {
    let owner = SourceCheckProjection::new(
        "fixture/authorized-source",
        7,
        source,
        conduit::source_check_worker::standard_context().unwrap(),
    )
    .unwrap();
    let request = SourceCheckRequest {
        snapshot: owner.snapshot().clone(),
        context: owner.context().clone(),
        entry: "hello".into(),
        limits: SourceCheckLimits::default(),
    };
    (
        SourceCheckProcessInput {
            source: source.into(),
            owner: "fixture/authorized-source".into(),
            generation: 7,
            request,
        },
        owner,
    )
}
fn child(bytes: &[u8], address_space: u64) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "source-check-worker",
            "--address-space-bytes",
            &address_space.to_string(),
            "--cpu-seconds",
            "10",
        ])
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // A contained OOM can close the pipe before the parent's write completes.
    let write = child.stdin.take().unwrap().write_all(bytes);
    let output = child.wait_with_output().unwrap();
    if output.status.success() {
        write.unwrap();
    }
    output
}
#[test]
fn actual_product_child_matches_direct_canonical_reference_for_valid_invalid_and_stale() {
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    for (source, scope_change) in [
        (include_str!("../../../plots/hello/main.conduit"), 0),
        ("# α 🦀\nplot hello { first: unavailable/kind }\n", 0),
        (include_str!("../../../plots/hello/main.conduit"), 1),
        (include_str!("../../../plots/hello/main.conduit"), 2),
    ] {
        let (mut input, mut owner) = input(source);
        if scope_change == 1 {
            input.request.snapshot.generation += 1;
        }
        if scope_change == 2 {
            input.request.context.catalog = "unselected/context".into();
        }
        let result = owner.project(&input.request);
        let expected = match &result {
            Ok(info) => SourceCheckReply::Checked(info),
            Err(reason) => SourceCheckReply::Refused(*reason),
        };
        let expected = schema.encode_reply(expected, 64 * 1024).unwrap();
        let output = child(&serde_json::to_vec(&input).unwrap(), 512 * 1024 * 1024);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected);
    }
}
#[test]
fn actual_product_child_refuses_extra_context_and_memory_pressure_without_reply() {
    let (input, _) = input(include_str!("../../../plots/hello/main.conduit"));
    let mut extra = serde_json::to_value(&input).unwrap();
    extra["catalogue"] = serde_json::json!({"claim": "not a compiler owner"});
    let output = child(&serde_json::to_vec(&extra).unwrap(), 512 * 1024 * 1024);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let output = child(&serde_json::to_vec(&input).unwrap(), 8 * 1024 * 1024);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("memory allocation")
            || error.contains("out of memory")
            || error.contains("Cannot allocate memory"),
        "{error}"
    );
}

#[test]
fn actual_product_child_rejects_unknown_nested_fields_and_oversized_startup() {
    let (input, _) = input(include_str!("../../../plots/hello/main.conduit"));
    for path in ["", "/snapshot", "/context", "/limits"] {
        let mut extra = serde_json::to_value(&input).unwrap();
        let target = if path.is_empty() {
            &mut extra["request"]
        } else {
            extra["request"].pointer_mut(path).unwrap()
        };
        target["unselected_authority"] = serde_json::json!(true);
        let output = child(&serde_json::to_vec(&extra).unwrap(), 512 * 1024 * 1024);
        assert!(!output.status.success(), "accepted unknown field at {path}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("unknown field"));
    }
    let oversized = vec![b' '; 6 * MAXIMUM_PLOT_SOURCE_BYTES + 16 * 1024 + 1];
    let output = child(&oversized, 512 * 1024 * 1024);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exceeds fixed byte profile"));
}

#[test]
fn kernel_reports_child_address_space_cpu_and_core_limits_before_source_arrives() {
    use std::time::{Duration, Instant};
    let bytes = 512 * 1024 * 1024;
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "source-check-worker",
            "--address-space-bytes",
            &bytes.to_string(),
            "--cpu-seconds",
            "10",
        ])
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Keep stdin open: no Source is available to check or reply to.
    let path = format!("/proc/{}/limits", child.id());
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed = false;
    while Instant::now() < deadline {
        if let Ok(limits) = std::fs::read_to_string(&path) {
            let exact = |name: &str, value: &str| {
                limits
                    .lines()
                    .find(|line| line.starts_with(name))
                    .is_some_and(|line| {
                        let mut fields = line.split_whitespace().rev();
                        fields.next();
                        fields.next() == Some(value) && fields.next() == Some(value)
                    })
            };
            observed = exact("Max address space", &bytes.to_string())
                && exact("Max cpu time", "10")
                && exact("Max core file size", "0");
            if observed {
                break;
            }
        }
        if matches!(child.try_wait(), Ok(Some(_))) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Always retire and reap, including a failed observation.
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        observed,
        "the actual child did not acquire the exact kernel limits"
    );
}

#[path = "source_check_process/parent.rs"]
mod parent;
