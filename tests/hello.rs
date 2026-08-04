use std::path::PathBuf;
use std::process::Command;

#[test]
fn hello_panel_runs_locally() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");

    let output = Command::new("cargo")
        .current_dir(&workspace_root)
        .args([
            "run",
            "--quiet",
            "-p",
            "conduit",
            "--",
            "examples/hello.panel",
        ])
        .output()
        .expect("failed to run conduit binary");

    assert!(output.status.success(), "process failed: {output:?}");

    let stdout = String::from_utf8(output.stdout).expect("stdout must be utf-8");
    assert_eq!(stdout.trim(), "Hello, world!");
}
