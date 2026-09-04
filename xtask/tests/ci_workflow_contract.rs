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
    assert!(required_gate.contains("if: ${{ always() && !cancelled() }}"));
    for aggregate in [
        "classify",
        "workspace-check",
        "esp32-firmware",
        "browser-host",
        "conduitos-limine",
        "conduitos-tools",
        "conduitos-x86",
        "conduitos-architecture",
        "conduitos-aarch64-product",
    ] {
        assert!(
            declared_join.contains(aggregate),
            "required check can finish without `{aggregate}`: {declared_join}"
        );
    }

    for result in [
        "BROWSER_HOST_RESULT",
        "LIMINE_RESULT",
        "TOOLS_RESULT",
        "X86_RESULT",
        "ARCHITECTURE_RESULT",
        "AARCH64_PRODUCT_RESULT",
    ] {
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
fn conduitos_result_join_is_folded_into_the_existing_final_gate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow =
        fs::read_to_string(root.join(".github/workflows/check.yml")).expect("read check workflow");
    assert!(!workflow.contains("\n  conduitos-boot:\n"));
    let join = workflow
        .split("\n  check:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  browser-tools:\n").next())
        .expect("locate final result gate");
    for result in [
        "LIMINE_RESULT",
        "TOOLS_RESULT",
        "X86_RESULT",
        "ARCHITECTURE_RESULT",
        "AARCH64_PRODUCT_RESULT",
    ] {
        assert!(
            join.contains(result),
            "ConduitOS join does not inspect `{result}`"
        );
    }
    assert!(join.contains("require_result conduitos-limine"));
    assert!(join.contains("runs-on: ubuntu-slim"));
    assert!(workflow.contains("name: Verify the final result gate truth table"));
    assert!(workflow.contains("node --test \"$proof\""));
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

    assert!(required_gate.contains("if: ${{ always() && !cancelled() }}"));
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

    for entry in fs::read_dir(&workflows).expect("read workflow directory") {
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
fn generic_ci_rust_toolchain_is_exact_and_matches_the_repository_default() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let toolchain =
        fs::read_to_string(root.join("rust-toolchain.toml")).expect("read Rust toolchain");
    assert!(toolchain.contains("channel = \"1.98.1\""));
    assert!(toolchain.contains("components = [\"clippy\", \"rustfmt\"]"));

    let workflows = root.join(".github/workflows");
    let mut exact_setups = 0;
    for entry in fs::read_dir(&workflows).expect("read workflow directory") {
        let path = entry.expect("read workflow entry").path();
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("yml" | "yaml")
        ) {
            continue;
        }
        let workflow = fs::read_to_string(&path).expect("read workflow");
        assert!(
            !workflow.contains("dtolnay/rust-toolchain@stable"),
            "{} resolves a moving generic Rust toolchain",
            path.display()
        );
        assert!(
            !workflow.contains("cargo +stable"),
            "{} overrides the exact repository toolchain with a moving channel",
            path.display()
        );
        exact_setups += workflow.matches("dtolnay/rust-toolchain@1.98.1").count();
    }
    assert!(exact_setups > 0, "expected exact generic Rust setup in CI");

    let registry = fs::read_to_string(root.join("xtask/src/commands/ci/proof_graph/spec.rs"))
        .expect("read proof registry");
    assert!(registry.contains("environment: \"ubuntu-rust-1.98.1-v1\""));
    assert!(!registry.contains("ubuntu-stable-rust"));
    let host_release = fs::read_to_string(root.join("xtask/src/commands/host_release.rs"))
        .expect("read Host release fabrication");
    assert!(
        !host_release.contains("+stable"),
        "Host release must inherit the exact repository toolchain"
    );

    let check = fs::read_to_string(workflows.join("check.yml")).expect("read check workflow");
    let products = fs::read_to_string(workflows.join("executable-book-pages.yml"))
        .expect("read product workflow");
    for workflow in [&check, &products] {
        let setup = workflow
            .find("dtolnay/rust-toolchain@1.98.1")
            .expect("exact toolchain setup");
        let components = workflow[setup..]
            .find("components: clippy,rustfmt")
            .map(|offset| setup + offset)
            .expect("exact generic components");
        let preflight = workflow
            .find("cargo +1.98.1 xtask ci rust-toolchain-preflight --locked")
            .expect("exact toolchain preflight");
        let planner = workflow.find("ci plan").expect("CI impact planner");
        assert!(setup < components && components < preflight && preflight < planner);
    }
}

#[test]
fn controller_failure_blocks_expensive_fanout_instead_of_selecting_everything() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let check =
        fs::read_to_string(root.join(".github/workflows/check.yml")).expect("read check workflow");
    let products = fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml"))
        .expect("read product workflow");

    assert!(!check.contains("needs.classify.result != 'success'"));
    for job in [
        "conduitos-limine",
        "conduitos-tools",
        "workspace-check",
        "standalone-locks",
        "conduitos-proof-image",
        "conduitos-x86",
        "conduitos-architecture",
        "conduitos-aarch64-product",
    ] {
        let start = check
            .find(&format!("  {job}:\n"))
            .unwrap_or_else(|| panic!("missing {job}"));
        let section = &check[start..check.len().min(start + 600)];
        assert!(
            section.contains("needs.classify.result == 'success'"),
            "{job} can fan out after classifier failure"
        );
    }
    assert!(products.contains(
        "if: always() && needs.plan.result == 'success' && needs.plan.outputs.required == 'true'"
    ));
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
fn browser_release_installs_its_exact_wasm_target() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml"))
        .expect("read product workflow");
    let browser_release = workflow
        .split("\n  browser-release:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  tour-patchbay-proof:\n").next())
        .expect("locate browser release job");
    assert!(browser_release.contains("targets: wasm32-unknown-unknown"));
}

#[test]
fn merged_branch_retirement_retargets_before_deletion_under_trusted_code() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/retire-merged-pr-branch.yml"))
        .expect("read merged branch retirement workflow");
    assert!(workflow.contains("pull_request_target:"));
    assert!(workflow.contains("types: [closed]"));
    assert!(workflow.contains("pull-requests: write"));
    assert!(workflow.contains("contents: write"));
    assert!(workflow.contains("ref: refs/heads/${{ github.event.repository.default_branch }}"));
    assert!(workflow.contains("persist-credentials: false"));
    assert!(workflow.contains("node scripts/ci/retire-merged-pr-branch.mjs"));
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
fn patchbay_debugger_has_one_authoritative_candidate_proof_node() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml"))
        .expect("read product workflow");
    let proof = workflow
        .split("\n  tour-patchbay-proof:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  avr-release:\n").next())
        .expect("locate shared focused browser proof environment");
    let gate = workflow
        .split("\n  products-proof:\n")
        .nth(1)
        .expect("locate stable product gate");

    assert!(!root
        .join(".github/workflows/patchbay-debugger-pr-proof.yml")
        .exists());
    assert!(proof.contains("ref: ${{ env.CONDUIT_CANDIDATE_SHA }}"));
    assert!(proof.contains("--workers 1"));
    assert!(proof.contains("--retries 0"));
    assert!(proof.contains("browser.patchbay-debugger"));
    assert!(proof.contains("ci-proof-browser.patchbay-debugger-${{ env.CONDUIT_CANDIDATE_SHA }}"));
    assert!(gate.contains("tour-patchbay-proof"));
    assert!(workflow.contains("id: debugger-bootstrap"));
    assert!(workflow.contains("reason=proof-definition-bootstrap"));
    assert!(workflow
        .contains("patchbay_debugger_required: ${{ steps.debugger-bootstrap.outputs.required }}"));
    assert_eq!(
        proof
            .matches("if: needs.plan.outputs.patchbay_debugger_required == 'true'")
            .count(),
        10
    );
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
    assert!(workflow.contains("  pull-requests: read\n"));
    assert!(workflow.contains("main_sha:\n"));
    assert!(workflow.contains("steps.resolve.outputs.direct_main == 'true'"));
    assert!(workflow.contains(
        "disposition: ${{ steps.direct.outputs.disposition || steps.reconcile.outputs.disposition }}"
    ));
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
    assert!(workflow.contains("integration_sha: ${{ steps.resolve.outputs.integration_sha }}"));
    assert!(workflow.contains("git merge-tree --write-tree \"$base_sha\" \"$CANDIDATE_SHA\""));
    assert!(workflow
        .contains("git commit-tree \"$integration_tree\" -p \"$base_sha\" -p \"$CANDIDATE_SHA\""));
    assert!(workflow.contains("git push origin \"$INTEGRATION_SHA:$INTEGRATION_REF\""));
    assert!(workflow.contains("git push origin \":$INTEGRATION_REF\""));
    assert!(workflow.contains("name: Classify candidate-to-integration proof impact"));
    let toolchain = workflow
        .find("name: Install the repository Rust toolchain for the impact controller")
        .expect("reconciliation installs the controller toolchain");
    let classify = workflow
        .find("name: Classify candidate-to-integration proof impact")
        .expect("reconciliation classifies integration impact");
    assert!(
        toolchain < classify,
        "the dependency-light impact controller cannot be built before Rust is installed"
    );
    assert!(workflow.contains("ci plan \"$CANDIDATE_SHA\" \"$INTEGRATION_SHA\" --locked"));
    assert!(workflow.contains("CONDUIT_RECONCILIATION_PLAN: reconciliation-impact.json"));
    assert_eq!(
        workflow
            .matches("candidate_sha: ${{ needs.resolve.outputs.integration_sha }}")
            .count(),
        2
    );
    assert!(workflow.contains("CONDUIT_CANDIDATE_SHA: ${{ inputs.candidate_sha }}"));
    assert!(
        workflow.contains("CONDUIT_INTEGRATION_SHA: ${{ needs.resolve.outputs.integration_sha }}")
    );
    assert!(workflow.contains("cancel-in-progress: false"));
    assert!(workflow.contains("resolve:\n    runs-on: ubuntu-slim\n    timeout-minutes: 5"));
    assert!(workflow.contains("uses: ./.github/workflows/check.yml"));
    assert!(workflow.contains("uses: ./.github/workflows/executable-book-pages.yml"));
    assert!(workflow.contains(
        "if: needs.resolve.result == 'success' && needs.resolve.outputs.check_inherited != 'true'"
    ));
    assert!(workflow.contains(
        "if: needs.resolve.result == 'success' && needs.resolve.outputs.products_inherited != 'true'"
    ));
    assert_eq!(workflow.matches("set -o pipefail").count(), 2);
    assert_eq!(workflow.matches("checks: write").count(), 2);
    assert!(workflow.contains("published: ${{ steps.resolve.outputs.published }}"));
    assert!(workflow.contains("report:\n    name: admission"));
    assert!(workflow.contains("if: always() && needs.resolve.outputs.integration_ref != ''"));
    assert!(!workflow.contains(
        "needs.resolve.outputs.integration_ref != '' && needs.resolve.outputs.published != 'true'"
    ));
    assert!(workflow.contains(
        "name: Publish the reconciliation-owned admission gate\n        if: needs.resolve.result == 'success'"
    ));
    assert!(workflow.contains("name: Publish the reconciliation-owned admission gate"));
    assert_eq!(workflow.matches("contents: write").count(), 2);
    assert!(workflow.contains("candidate-check:\n    needs: resolve"));
    assert!(workflow.contains(
        "candidate-check:\n    needs: resolve\n    if: needs.resolve.result == 'success' && needs.resolve.outputs.check_inherited != 'true'\n    permissions:\n      actions: read\n      contents: read\n      pull-requests: read"
    ));
    assert!(workflow.contains("candidate-products:\n    needs: resolve"));
    assert!(workflow.contains(
        "candidate-products:\n    needs: resolve\n    if: needs.resolve.result == 'success' && needs.resolve.outputs.products_inherited != 'true'\n    permissions:\n      contents: read\n      pull-requests: read"
    ));

    assert!(check.contains("workflow_call:"));
    assert!(check.contains(
        "CONDUIT_CHECKOUT_SHA: ${{ inputs.candidate_sha || github.event.pull_request.head.sha"
    ));
    assert!(!check.contains("name: conduitos-limine-${{ github.sha }}"));
}

#[test]
fn new_product_proofs_are_attested_by_the_trusted_controller_against_candidate_bytes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml"))
        .expect("read product workflow");

    assert!(workflow.contains("name: Materialize the trusted attestation controller"));
    assert!(workflow.contains("CONTROLLER_SHA: ${{ needs.plan.outputs.controller_sha }}"));
    assert!(workflow.contains(
        "git -c safe.directory=\"$GITHUB_WORKSPACE\" fetch --no-tags origin \"$CONTROLLER_SHA\""
    ));
    assert!(workflow.contains("\"$RUNNER_TEMP/conduit-ci-controller-target/debug/conduit-xtask-dispatch\"\n          ci attest-success \"$CONDUIT_CANDIDATE_SHA\""));
    assert!(!workflow.contains("cargo xtask ci attest-success \"$CONDUIT_CANDIDATE_SHA\"\n          browser.patchbay-debugger"));
}

#[test]
fn candidate_lifecycle_controllers_use_the_lightweight_automation_lane() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let reconciliation = fs::read_to_string(root.join(".github/workflows/reconcile-candidate.yml"))
        .expect("read candidate reconciliation workflow");
    let retirement =
        fs::read_to_string(root.join(".github/workflows/retire-superseded-candidates.yml"))
            .expect("read candidate retirement workflow");

    assert_eq!(reconciliation.matches("runs-on: ubuntu-slim").count(), 2);
    assert!(!reconciliation.contains("runs-on: ubuntu-latest"));
    assert_eq!(retirement.matches("runs-on: ubuntu-slim").count(), 1);
    assert!(!retirement.contains("runs-on: ubuntu-latest"));
    assert_eq!(reconciliation.matches("checks: write").count(), 2);
    assert_eq!(reconciliation.matches("contents: write").count(), 2);
    assert!(reconciliation.contains("timeout-minutes: 2"));
    assert!(retirement.contains("timeout-minutes: 2"));
}
