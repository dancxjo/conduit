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
        "conduit-copy-face-{label}-{}-{nonce}",
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
    assert!(
        stdout.contains("Presentation: type=structured-info/profile-"),
        "{stdout}"
    );
    let inspected = &stdout[inspect..];
    assert!(inspected.contains("task: file/copy"), "{inspected}");
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
        inspected.contains("gear: copy-task/task (face: 0 inputs, 1 outputs)"),
        "{inspected}"
    );
    assert!(
        inspected.contains("gear: copy-task/show (face: 1 inputs, 0 outputs)"),
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

#[test]
fn copy_options_are_accepted_before_between_and_after_paths() {
    let directory = test_directory("option-order");
    let source = directory.join("source.txt");
    fs::write(&source, b"ordered options").unwrap();
    let source = source.to_str().unwrap();

    for (index, arguments) in [
        vec!["--inspect", "--run", source, "DESTINATION"],
        vec![source, "--inspect", "--run", "DESTINATION"],
        vec![source, "DESTINATION", "--inspect", "--run"],
    ]
    .into_iter()
    .enumerate()
    {
        let destination = directory.join(format!("inspect-{index}.txt"));
        let destination = destination.to_str().unwrap();
        let arguments = arguments
            .into_iter()
            .map(|argument| {
                if argument == "DESTINATION" {
                    destination
                } else {
                    argument
                }
            })
            .collect::<Vec<_>>();
        let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("copy")
            .args(arguments)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(fs::read(destination).unwrap(), b"ordered options");
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("Inspect (after the task)"),
            "ordering {index} omitted inspection"
        );
    }

    for (index, arguments) in [
        vec![
            "--mode",
            "replace",
            "--max-bytes",
            "64",
            source,
            "DESTINATION",
            "--run",
        ],
        vec![
            source,
            "--mode",
            "replace",
            "DESTINATION",
            "--max-bytes",
            "64",
            "--run",
        ],
        vec![
            source,
            "DESTINATION",
            "--mode",
            "replace",
            "--max-bytes",
            "64",
            "--run",
        ],
    ]
    .into_iter()
    .enumerate()
    {
        let destination = directory.join(format!("values-{index}.txt"));
        fs::write(&destination, b"old").unwrap();
        let destination = destination.to_str().unwrap();
        let arguments = arguments
            .into_iter()
            .map(|argument| {
                if argument == "DESTINATION" {
                    destination
                } else {
                    argument
                }
            })
            .collect::<Vec<_>>();
        let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("copy")
            .args(arguments)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(fs::read(destination).unwrap(), b"ordered options");
    }

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn malformed_copy_arguments_fail_closed_with_specific_usage_diagnostics() {
    let cases: &[(&[&str], &str)] = &[
        (
            &["SOURCE", "DESTINATION", "--wat"],
            "unknown option '--wat'",
        ),
        (
            &["SOURCE", "DESTINATION", "--inspect", "--inspect"],
            "option '--inspect' was repeated",
        ),
        (
            &[
                "--mode",
                "create",
                "SOURCE",
                "--mode",
                "replace",
                "DESTINATION",
            ],
            "option '--mode' was repeated",
        ),
        (
            &[
                "SOURCE",
                "DESTINATION",
                "--max-bytes",
                "8",
                "--max-bytes",
                "9",
            ],
            "option '--max-bytes' was repeated",
        ),
        (
            &["SOURCE", "DESTINATION", "--run", "--run"],
            "option '--run' was repeated",
        ),
        (
            &["SOURCE", "DESTINATION", "--mode"],
            "option '--mode' requires a value (create|replace)",
        ),
        (
            &["SOURCE", "--max-bytes", "--inspect", "DESTINATION"],
            "option '--max-bytes' requires a value (N)",
        ),
        (
            &["SOURCE", "DESTINATION", "--mode", "overwrite"],
            "invalid value 'overwrite' for --mode; expected create or replace",
        ),
        (
            &["SOURCE", "DESTINATION", "--max-bytes", "many"],
            "invalid value 'many' for --max-bytes; expected a positive integer",
        ),
        (
            &["SOURCE", "DESTINATION", "--max-bytes", "0"],
            "invalid value '0' for --max-bytes; expected 1..=16777216",
        ),
        (
            &[],
            "missing required positional operands SOURCE and DESTINATION",
        ),
        (
            &["SOURCE", "--inspect"],
            "missing required positional operand DESTINATION",
        ),
        (
            &["SOURCE", "DESTINATION", "EXTRA"],
            "unexpected positional operand 'EXTRA'; exactly SOURCE and DESTINATION are required",
        ),
    ];

    for (arguments, expected) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("copy")
            .args(*arguments)
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "{arguments:?} unexpectedly passed"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(expected), "{arguments:?}: {stderr}");
        assert!(
            stderr.contains("usage: conduit copy [OPTIONS] SOURCE DESTINATION"),
            "{arguments:?}: {stderr}"
        );
    }
}
