use std::fs;
use std::path::PathBuf;

#[test]
fn required_check_waits_for_every_selectable_proof_aggregate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow =
        fs::read_to_string(root.join(".github/workflows/check.yml")).expect("read check workflow");
    let required_gate = workflow
        .split("\n  check:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  browser-tools:\n").next())
        .expect("locate the stable required check job");

    let declared_join = required_gate
        .lines()
        .find(|line| line.trim_start().starts_with("needs:"))
        .expect("required check declares its proof join");
    for aggregate in [
        "classify",
        "workspace-check",
        "esp32-firmware",
        "browser-host",
        "conduitos-boot",
    ] {
        assert!(
            declared_join.contains(aggregate),
            "required check can finish without `{aggregate}`: {declared_join}"
        );
    }

    for result in ["BROWSER_HOST_RESULT", "CONDUITOS_RESULT"] {
        assert!(
            required_gate.contains(result),
            "required check joins the job but does not inspect `{result}`"
        );
    }
}

#[test]
fn classifier_artifacts_and_cache_are_scoped_away_from_the_controller() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow =
        fs::read_to_string(root.join(".github/workflows/check.yml")).expect("read check workflow");

    assert!(workflow.contains("--json-out target/ci-plans/ci-impact-plan.json"));
    assert!(workflow.contains("--json-out target/ci-plans/ci-proof-plan.json"));
    assert!(workflow.contains("path: target/ci-plans"));
    assert!(!workflow.contains("path: |\n            target/ci-impact-plan.json"));
    assert!(workflow.contains("$RUNNER_TEMP/conduit-ci-controller"));
    assert!(workflow.contains("$RUNNER_TEMP/conduit-ci-controller-target"));
    assert!(!workflow.contains("target/ci-controller"));
}

#[test]
fn stacked_diff_base_does_not_select_the_controller_version() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow =
        fs::read_to_string(root.join(".github/workflows/check.yml")).expect("read check workflow");

    assert!(workflow.contains("name: Resolve the current trusted CI controller"));
    assert!(workflow.contains("DEFAULT_BRANCH: ${{ github.event.repository.default_branch }}"));
    assert!(workflow.contains("git ls-remote --exit-code origin"));
    assert!(workflow.contains(
        "git worktree add --detach \"$RUNNER_TEMP/conduit-ci-controller\" \"$CONTROLLER_SHA\""
    ));
    assert!(!workflow.contains("name: Check out the trusted versioned CI controller"));

    // The stacked target remains the exact diff base. Only controller policy
    // comes from the current trusted default branch.
    assert!(workflow.contains(
        "BASE_SHA: ${{ github.event.pull_request.base.sha || github.event.merge_group.base_sha || github.event.before }}"
    ));
    assert!(workflow.contains("controller_sha: ${{ steps.controller.outputs.sha }}"));
}
