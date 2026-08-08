use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_directory(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must follow the Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "conduit-copy-front-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("test directory should be creatable");
    path
}

#[test]
fn copy_task_runs_then_reveals_path_free_form_and_exact_plan() {
    let directory = test_directory("success");
    let source = directory.join("chosen-source.txt");
    let destination = directory.join("chosen-destination.txt");
    fs::write(&source, b"bounded copy\n").expect("source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "copy",
            source.to_str().expect("UTF-8 source path"),
            destination.to_str().expect("UTF-8 destination path"),
            "--run",
            "--inspect",
        ])
        .output()
        .expect("copy task should launch");

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read(&destination).expect("destination should exist"),
        b"bounded copy\n"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let ready = stdout
        .find("Ready: yes")
        .expect("readiness should be shown");
    let result = stdout
        .find("Result: Copied 13 bytes successfully.")
        .expect("success should be shown");
    let inspect = stdout
        .find("Inspect (after the task)")
        .expect("Inspect should follow execution");
    assert!(ready < result && result < inspect, "{stdout}");
    let inspected = &stdout[inspect..];
    assert!(inspected.contains("copy: file/copy"), "{inspected}");
    let plan_line = inspected
        .lines()
        .find(|line| line.starts_with("Plan: "))
        .expect("Plan identity should be shown");
    assert_eq!(
        plan_line.trim_start_matches("Plan: ").len(),
        64,
        "{inspected}"
    );
    assert!(
        inspected.contains("operation: copy (face: 0 inputs, 0 outputs)"),
        "{inspected}"
    );
    assert!(!inspected.contains(source.to_str().unwrap()), "{inspected}");
    assert!(
        !inspected.contains(destination.to_str().unwrap()),
        "{inspected}"
    );

    fs::remove_dir_all(directory).expect("test directory should be removable");
}

#[test]
fn create_mode_reports_destination_exists_without_replacing_it() {
    let directory = test_directory("exists");
    let source = directory.join("source.txt");
    let destination = directory.join("destination.txt");
    fs::write(&source, b"new").expect("source should be writable");
    fs::write(&destination, b"old").expect("destination should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "copy",
            source.to_str().expect("UTF-8 source path"),
            destination.to_str().expect("UTF-8 destination path"),
            "--mode",
            "create",
            "--run",
        ])
        .output()
        .expect("copy task should launch");

    assert!(output.status.success(), "{output:?}");
    assert_eq!(fs::read(&destination).unwrap(), b"old");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(
        stdout.contains("Behavior: Create new; reject if the destination already exists"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Result: Not copied: destination already exists."),
        "{stdout}"
    );

    fs::remove_dir_all(directory).expect("test directory should be removable");
}

#[test]
fn task_does_not_copy_until_run_is_confirmed() {
    let directory = test_directory("quit");
    let source = directory.join("source.txt");
    let destination = directory.join("destination.txt");
    fs::write(&source, b"not yet").expect("source should be writable");

    let mut child = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "copy",
            source.to_str().expect("UTF-8 source path"),
            destination.to_str().expect("UTF-8 destination path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("copy task should launch");
    use std::io::Write;
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(b"quit\n")
        .expect("quit should be writable");
    let output = child.wait_with_output().expect("task should finish");

    assert!(output.status.success(), "{output:?}");
    assert!(!destination.exists());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("type 'run' to Run"), "{stdout}");
    assert!(stdout.contains("No copy was run."), "{stdout}");

    fs::remove_dir_all(directory).expect("test directory should be removable");
}
