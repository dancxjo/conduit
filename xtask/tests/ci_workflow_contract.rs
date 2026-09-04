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
fn classifier_artifacts_are_scoped_and_planning_has_no_workspace_cache() {
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

    let classifier = workflow
        .split("\n  classify:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  conduitos-limine:\n").next())
        .expect("locate classifier job");
    assert!(classifier.contains("package conduit-xtask-dispatch --locked"));
    assert!(
        !classifier.contains("Swatinem/rust-cache"),
        "the dependency-light planner must not restore or upload the workspace target cache"
    );
}

#[test]
fn product_planner_has_no_workspace_cache() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml"))
        .expect("read product workflow");
    let planner = workflow
        .split("\n  plan:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  browser-runtimes:\n").next())
        .expect("locate product planner job");

    assert!(planner.contains("package conduit-xtask-dispatch --locked"));
    assert!(
        !planner.contains("Swatinem/rust-cache"),
        "the dependency-light product planner must not transfer the workspace target cache"
    );
}

#[test]
fn product_stage_joins_exact_required_results_after_optional_skips() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml"))
        .expect("read product workflow");
    let stage = workflow
        .split("\n  products-stage:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  browser-proof:\n").next())
        .expect("locate product staging job");

    assert!(stage.contains("if: >-\n      always() &&"));
    for prerequisite in [
        "avr-release",
        "browser-release",
        "browser-runtimes",
        "esp32-release-images",
        "host-releases",
        "orange-pi-release",
        "raspberry-pi-release",
        "conduitos-releases",
    ] {
        assert!(stage.contains(&format!("needs.{prerequisite}.result == 'success'")));
    }
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

#[test]
fn tour_proof_has_one_authoritative_candidate_workflow() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    assert!(!root.join(".github/workflows/book-pr-proof.yml").exists());
    let workflow = fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml"))
        .expect("read product workflow");
    let proof = workflow
        .split("\n  tour-patchbay-proof:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  avr-release:\n").next())
        .expect("locate authoritative early Tour proof");
    assert!(proof.contains("ref: ${{ env.CONDUIT_CANDIDATE_SHA }}"));
    assert!(proof.contains("node --test"));
    assert!(proof.contains("proof/browser/patchbay-debugger-projection.test.mjs"));
    assert!(proof.contains("proof/browser/playwright-config.test.mjs"));
    assert!(proof.contains("real Patchbay renderer|animated Cords"));
}

#[test]
fn pages_promotion_verifies_candidate_provenance_after_input_reconciliation() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/executable-book-deploy.yml"))
        .expect("read Pages deployment workflow");

    assert!(workflow.contains("refs/pull/$PR_NUMBER/head"));
    assert!(workflow.contains("test \"$(git rev-parse FETCH_HEAD)\" = \"$SOURCE_HEAD\""));
    assert!(workflow.contains("reconcile-product \\"));
    assert!(workflow.contains("products.pages-carrier \"$SOURCE_HEAD\" \"$MERGE_COMMIT\""));
    assert!(workflow.contains("needs.resolve.outputs.disposition == 'execute'"));
    assert!(workflow.contains("uses: ./.github/workflows/executable-book-pages.yml"));
    assert!(workflow.contains("candidate_sha: ${{ needs.resolve.outputs.merge_commit }}"));
    assert!(workflow.contains("name: Download the inherited candidate Pages carrier"));
    assert!(workflow.contains("name: Download the newly proven integration Pages carrier"));
    assert!(workflow.contains(
        "needs.resolve.outputs.disposition == 'inherited' && needs.resolve.outputs.source_tree || needs.resolve.outputs.integration_tree"
    ));

    let products = fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml"))
        .expect("read product workflow");
    assert!(products.contains(
        "CONDUIT_CANDIDATE_SHA: ${{ inputs.candidate_sha || github.event.pull_request.head.sha }}"
    ));
    assert!(products.contains(
        "CONDUIT_BASE_SHA: ${{ inputs.base_sha || github.event.pull_request.base.sha }}"
    ));
    assert!(!products.contains("github.event.pull_request.head.sha || inputs.candidate_sha"));
    assert!(!products.contains("github.event.pull_request.base.sha || inputs.base_sha"));
    for source in [&workflow, &products] {
        assert!(!source.contains("actions/upload-artifact@v4"));
        assert!(!source.contains("actions/download-artifact@v6"));
    }
    assert!(workflow.contains("actions/download-artifact@v8"));
    assert!(products.contains("actions/upload-artifact@v7"));
    assert!(products.contains("actions/download-artifact@v8"));
}
