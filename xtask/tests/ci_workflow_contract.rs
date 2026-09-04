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

    assert!(
        required_gate.contains("runs-on: ubuntu-slim"),
        "the terminal gate must not compete with heavyweight proof runners"
    );
}

#[test]
fn required_product_gate_uses_the_lightweight_automation_lane() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml"))
        .expect("read product workflow");
    let required_gate = workflow
        .split("\n  products-proof:\n")
        .nth(1)
        .expect("locate the stable required product job");

    assert!(required_gate.contains("if: always()"));
    assert!(required_gate.contains("runs-on: ubuntu-slim"));
    for result in [
        "TOUR_PATCHBAY_RESULT",
        "STAGE_RESULT",
        "BROWSER_RESULT",
        "CARRIER_RESULT",
    ] {
        assert!(
            required_gate.contains(result),
            "product gate does not inspect `{result}`"
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
fn rust_target_caches_never_save_after_a_failed_proof() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflows = root.join(".github/workflows");
    let mut cache_uses = 0;
    let mut explicit_fail_closed_policies = 0;

    for entry in fs::read_dir(workflows).expect("read workflow directory") {
        let path = entry.expect("read workflow entry").path();
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("yml" | "yaml")
        ) {
            continue;
        }
        let workflow = fs::read_to_string(&path).expect("read workflow");
        cache_uses += workflow.matches("uses: Swatinem/rust-cache@v2").count();
        explicit_fail_closed_policies += workflow.matches("cache-on-failure: false").count();
        assert!(
            !workflow.contains("cache-on-failure: true"),
            "{} permits a deterministic proof failure to upload a heavyweight Rust cache",
            path.display()
        );
    }

    assert!(cache_uses > 0, "expected at least one Rust cache consumer");
    assert_eq!(
        explicit_fail_closed_policies, cache_uses,
        "every Rust cache consumer must state the no-save-on-failure policy explicitly"
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
fn product_descendants_use_explicit_direct_result_admission() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml"))
        .expect("read product workflow");
    let browser = workflow
        .split("\n  browser-proof:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  pages-carrier:\n").next())
        .expect("locate browser proof job");
    let carrier = workflow
        .split("\n  pages-carrier:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  products-proof:\n").next())
        .expect("locate Pages carrier job");

    assert!(browser.contains("if: always() && needs.products-stage.result == 'success'"));
    assert!(carrier.contains(
        "if: always() && needs.products-stage.result == 'success' && needs.browser-proof.result == 'success'"
    ));
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
        "BASE_SHA: ${{ inputs.base_sha || github.event.pull_request.base.sha || github.event.merge_group.base_sha || github.event.before }}"
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

#[test]
fn pages_resolver_has_one_local_and_hosted_proof_entrance() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/pages-deploy-pr-proof.yml"))
        .expect("read Pages resolver proof workflow");
    let dispatcher =
        fs::read_to_string(root.join("tools/xtask-dispatch/src/ci_dispatch/pages_resolver.rs"))
            .expect("read dependency-light Pages resolver command");

    assert!(workflow.contains("cargo xtask ci pages-resolver-proof --locked"));
    assert!(!workflow.contains("node --test proof/ci/"));
    for proof in [
        "proof/ci/pages-product-run-selection.spec.mjs",
        "proof/ci/pages-workflow-paths.spec.mjs",
    ] {
        assert_eq!(
            dispatcher.matches(proof).count(),
            1,
            "resolver proof input must be owned once: {proof}"
        );
    }
}

#[test]
fn unchanged_candidate_reconciliation_is_exact_head_and_least_privilege() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/reconcile-candidate.yml"))
        .expect("read candidate reconciliation workflow");
    let check =
        fs::read_to_string(root.join(".github/workflows/check.yml")).expect("read check workflow");

    assert!(workflow.contains(
        "group: reconcile-candidate-${{ inputs.pr_number }}-${{ inputs.candidate_sha }}"
    ));
    assert!(workflow.contains("cancel-in-progress: false"));
    assert!(workflow.contains("uses: ./.github/workflows/check.yml"));
    assert!(workflow.contains("uses: ./.github/workflows/executable-book-pages.yml"));
    assert!(workflow.contains("if: needs.resolve.outputs.check_inherited != 'true'"));
    assert!(workflow.contains("if: needs.resolve.outputs.products_inherited != 'true'"));
    assert_eq!(workflow.matches("checks: write").count(), 1);
    assert!(workflow.contains("name: Publish stable required checks on the unchanged candidate"));
    assert!(workflow.contains("persist-credentials: false"));

    assert!(check.contains("workflow_call:"));
    assert!(check.contains(
        "CONDUIT_CHECKOUT_SHA: ${{ inputs.candidate_sha || github.event.pull_request.head.sha"
    ));
    assert!(!check.contains("name: conduitos-limine-${{ github.sha }}"));
}
