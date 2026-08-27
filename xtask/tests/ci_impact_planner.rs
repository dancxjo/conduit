use std::path::PathBuf;
use std::process::Command;

#[test]
fn heavyweight_ci_impact_planner_has_safe_representative_decisions() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must live under the repository root")
        .to_path_buf();
    let planner = repository.join("scripts/ci/plan-heavy-impact.py");

    let output = Command::new("python3")
        .arg(&planner)
        .arg("--self-test")
        .current_dir(&repository)
        .output()
        .expect("python3 must launch the repository CI impact planner");

    assert!(
        output.status.success(),
        "CI impact planner self-test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("impact-planner-self-test: ok"),
        "CI impact planner did not emit its exact success marker"
    );
}
