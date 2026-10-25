use super::*;
fn bounds() -> BuilderBounds {
    BuilderBounds {
        maximum_source_files: 10,
        maximum_source_bytes: 1024,
        maximum_workspace_bytes: 1024 * 1024,
        maximum_output_bytes: 1024,
        maximum_processes: 16,
        maximum_seconds: 2,
        maximum_evidence_bytes: 128,
    }
}
#[test]
fn output_pressure_and_cancellation_are_distinct() {
    let root = std::env::temp_dir().join(format!("conduit-linux-process-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let bounds = bounds();
    let cancelled = AtomicBool::new(false);
    let mut execution = Execution::new(&root, None, &bounds, &cancelled);
    let mut command = Command::new("printf");
    command.arg("x".repeat(256));
    assert!(matches!(
        execution.run(&mut command),
        Err(BuilderRefusal::BoundExhausted)
    ));
    assert_eq!(
        fs::read(root.join("execution-000001-stdout.log")).unwrap(),
        vec![b'x'; 128]
    );
    assert_eq!(
        fs::metadata(root.join("execution-000001-stderr.log"))
            .unwrap()
            .len(),
        0
    );
    cancelled.store(true, Ordering::Release);
    assert!(matches!(
        execution.run(&mut Command::new("true")),
        Err(BuilderRefusal::Interrupted)
    ));
    fs::remove_dir_all(&root).unwrap();
}
#[test]
fn cancellation_terminates_a_running_command() {
    let root = std::env::temp_dir().join(format!("conduit-linux-cancel-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let bounds = bounds();
    let cancelled = Arc::new(AtomicBool::new(false));
    let trigger = cancelled.clone();
    let cancellation = thread::spawn(move || {
        thread::sleep(Duration::from_millis(75));
        trigger.store(true, Ordering::Release);
    });
    let mut execution = Execution::new(&root, None, &bounds, &cancelled);
    let mut command = Command::new("sleep");
    command.arg("30");
    let started = Instant::now();
    assert!(matches!(
        execution.run(&mut command),
        Err(BuilderRefusal::Interrupted)
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
    cancellation.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn failing_command_retains_exact_stderr_without_overwrite() {
    let root = std::env::temp_dir().join(format!("conduit-linux-stderr-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let bounds = bounds();
    let cancelled = AtomicBool::new(false);
    let mut execution = Execution::new(&root, None, &bounds, &cancelled);
    let mut command = Command::new("sh");
    command.args(["-c", "printf reviewed-build-error >&2; exit 7"]);
    assert!(matches!(
        execution.run(&mut command),
        Err(BuilderRefusal::BuildFailed)
    ));
    let diagnostic = root.join("execution-000001-stderr.log");
    assert_eq!(fs::read(&diagnostic).unwrap(), b"reviewed-build-error");
    // A new execution cannot silently replace retained proof from an earlier one.
    let mut duplicate = Execution::new(&root, None, &bounds, &cancelled);
    assert!(matches!(
        duplicate.run(&mut Command::new("true")),
        Err(BuilderRefusal::WorkspaceUnavailable)
    ));
    assert_eq!(fs::read(diagnostic).unwrap(), b"reviewed-build-error");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn accounting_tolerates_disappearing_children_but_preserves_real_errors() {
    let root =
        std::env::temp_dir().join(format!("conduit-linux-accounting-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("cargo-temporary"), b"temporary").unwrap();
    let mut removed = false;
    let bytes = directory_bytes_observing(&root, 1024, &mut |path| {
        // Deterministic actual disappearance between readdir and metadata.
        fs::remove_file(path).unwrap();
        removed = true;
    })
    .unwrap();
    assert!(removed);
    assert_eq!(bytes, 0);
    let file = root.join("not-a-directory");
    fs::write(&file, b"retained").unwrap();
    assert_eq!(
        directory_bytes(&file, 1024),
        Err(BuilderRefusal::WorkspaceUnavailable)
    );
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(
        directory_bytes(&root, 1024),
        Err(BuilderRefusal::WorkspaceUnavailable)
    );
}
#[test]
fn process_group_stat_parser_counts_this_process() {
    // SAFETY: getpgrp has no preconditions and does not mutate process state.
    assert!(process_group_size(unsafe { libc::getpgrp() }).unwrap() >= 1);
}

#[test]
fn free_space_floor_refuses_before_spawn_without_filling_disk() {
    let root =
        std::env::temp_dir().join(format!("conduit-linux-free-floor-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let bounds = bounds();
    let cancelled = AtomicBool::new(false);
    let current = available_bytes(&root).unwrap();
    let impossible = u64::MAX;
    assert!(impossible > current); // Cannot race another owner's ordinary disk cleanup.
    let mut execution =
        Execution::new(&root, None, &bounds, &cancelled).minimum_free_bytes(impossible);
    assert!(matches!(
        execution.run(&mut Command::new("true")),
        Err(BuilderRefusal::BoundExhausted)
    ));
    assert_eq!(
        fs::read_dir(&root).unwrap().count(),
        0,
        "no process diagnostic files created before admission"
    );
    let mut ordinary = Execution::new(&root, None, &bounds, &cancelled);
    ordinary.run(&mut Command::new("true")).unwrap();
    assert_eq!(
        available_bytes(&root.join("missing")),
        Err(BuilderRefusal::WorkspaceUnavailable)
    );
    fs::remove_dir_all(root).unwrap();
}
