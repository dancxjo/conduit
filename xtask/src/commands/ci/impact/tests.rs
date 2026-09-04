use super::*;

#[test]
fn representative_changes_select_only_owned_heavy_suites() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();
    let cases = [
        (
            vec!["README.md", "docs/architecture/foo.md"],
            (false, false, false),
        ),
        (vec!["bodies/pete/src/lib.rs"], (false, false, false)),
        (
            vec!["targets/esp32/firmware/c3-signal/src/main.rs"],
            (true, false, false),
        ),
        (vec!["proof/browser/pointer.spec.mjs"], (false, true, false)),
        (vec!["targets/conduitos/src/main.rs"], (false, false, true)),
        (vec!["architecture/kernel/src/lib.rs"], (true, true, true)),
        (vec![".github/workflows/check.yml"], (true, true, true)),
        (vec!["unknown/new-input.bin"], (true, true, true)),
    ];
    for (paths, expected) in cases {
        let paths = paths.into_iter().map(str::to_owned).collect();
        let plan = plan_for_paths(&root, paths, &packages).unwrap();
        assert_eq!(
            (
                plan.esp32_required,
                plan.browser_required,
                plan.conduitos_required
            ),
            expected
        );
    }
}

#[test]
fn candidate_retirement_controller_changes_run_only_their_exact_proof() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();
    let plan = plan_for_paths(
        &root,
        vec![
            ".github/workflows/retire-superseded-candidates.yml".to_owned(),
            "proof/ci/retire-superseded-candidates.spec.mjs".to_owned(),
            "scripts/ci/retire-superseded-candidates.mjs".to_owned(),
        ],
        &packages,
    )
    .unwrap();

    assert_eq!(plan.ci_controller_proofs, ["ci.candidate-retirement"]);
    assert!(!plan.full_fallback);
    assert!(!plan.browser_required);
    assert!(!plan.esp32_required);
    assert!(!plan.conduitos_required);
    assert!(plan.workspace_shards.values().all(|required| !required));

    let mixed = plan_for_paths(
        &root,
        vec![
            "scripts/ci/retire-superseded-candidates.mjs".to_owned(),
            "unclassified-controller-neighbor.bin".to_owned(),
        ],
        &packages,
    )
    .unwrap();
    assert!(mixed.full_fallback);
    assert_eq!(mixed.ci_controller_proofs, ["ci.candidate-retirement"]);
}

#[test]
fn pages_products_follow_the_typed_live_ownership_registry() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();
    for path in [
        "products/tour/assets/tour.mjs",
        "products/patchbay/html/assets/app.js",
    ] {
        let plan = plan_for_paths(&root, vec![path.to_owned()], &packages).unwrap();
        assert!(plan.pages_products_required, "{path}");
        assert_eq!(plan.pages_product_proofs, ["products.pages-carrier"]);
    }

    for path in [
        "docs/architecture/example.md",
        "proof/browser/reviewed-form-conformance.spec.mjs",
        "xtask/src/commands/forms/browser.rs",
    ] {
        let plan = plan_for_paths(&root, vec![path.to_owned()], &packages).unwrap();
        assert!(!plan.pages_products_required, "{path}");
        assert!(plan.pages_product_proofs.is_empty(), "{path}");
    }
}

#[test]
fn registered_form_commands_do_not_fabricate_unrelated_machines() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();
    let plan = plan_for_paths(
        &root,
        vec![
            "forms/inventory.toml".to_owned(),
            "xtask/src/commands/forms.rs".to_owned(),
            "xtask/src/commands/forms/deterministic.rs".to_owned(),
        ],
        &packages,
    )
    .unwrap();

    assert_eq!(plan.repository_command_proofs, ["repository.forms"]);
    assert!(!plan.full_fallback);
    assert!(plan.browser_required);
    assert!(!plan.esp32_required);
    assert!(!plan.conduitos_required);
    assert!(plan.changed_packages.contains(&"xtask".to_owned()));
    assert!(plan.workspace_shards["lint"]);
    assert!(plan.workspace_shards["test-products"]);

    let cross_product = plan_for_paths(
        &root,
        vec!["xtask/src/commands/host_release.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(cross_product.full_fallback);
    assert!(cross_product.esp32_required);
    assert!(cross_product.browser_required);
    assert!(cross_product.conduitos_required);
}

#[test]
fn complete_tongues_analysis_slice_avoids_unrelated_machine_fabrication() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();
    let paths = [
        "semantics/tongues/src/analysis.rs",
        "semantics/tongues/tests/dynamics_analysis.rs",
        "forms/tongues-dynamics-analysis/main.conduit",
        "products/patchbay/html/src/learned_demo.rs",
        "proof/browser/patchbay-debugger-watch.spec.mjs",
        "xtask/src/cli.rs",
        "xtask/src/main.rs",
        "xtask/src/commands/tongues.rs",
    ]
    .map(str::to_owned)
    .to_vec();
    let plan = plan_for_paths(&root, paths, &packages).unwrap();
    assert!(!plan.full_fallback);
    assert!(plan.browser_required);
    assert!(!plan.esp32_required);
    assert!(!plan.conduitos_required);
    assert!(plan.changed_packages.contains(&"conduit-tongues".into()));
    assert!(plan.changed_packages.contains(&"patchbay-html".into()));
    assert!(plan.changed_packages.contains(&"xtask".into()));
}

#[test]
fn acceptance_diff_classes_keep_exact_obligation_boundaries() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();

    let docs = plan_for_paths(
        &root,
        vec!["docs/ci-impact-benchmark.md".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(!docs.esp32_required);
    assert!(!docs.browser_required);
    assert!(!docs.conduitos_required);
    assert!(docs.workspace_shards.values().all(|required| !required));

    let patchbay = plan_for_paths(
        &root,
        vec!["products/patchbay/model/src/lib.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(!patchbay.esp32_required);
    assert!(patchbay.browser_required);
    assert!(!patchbay.conduitos_required);

    let semantic = plan_for_paths(
        &root,
        vec!["semantics/language/src/lib.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(semantic
        .affected_test_packages
        .contains(&"conduit-std-host".to_owned()));
    assert!(semantic
        .affected_test_packages
        .contains(&"conduit".to_owned()));
    assert!(!semantic.esp32_required);
    assert!(semantic.browser_required);
    assert!(!semantic.conduitos_required);
    assert!(semantic.workspace_shards["portable"]);
    assert!(!semantic.workspace_shards["pico"]);

    let scientific_data = plan_for_paths(
        &root,
        vec!["semantics/data/src/scientific_observation.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(!scientific_data.full_fallback);
    assert!(!scientific_data.esp32_required);
    assert!(scientific_data.browser_required);
    assert!(!scientific_data.conduitos_required);
    assert!(scientific_data.workspace_shards["lint"]);
    assert!(scientific_data.workspace_shards["portable"]);
    assert!(scientific_data.workspace_shards["pico"]);
    assert!(scientific_data.workspace_shards["test-foundation"]);
    assert!(scientific_data.workspace_shards["test-hosts"]);
    assert!(scientific_data.workspace_shards["test-products"]);

    let browser = plan_for_paths(
        &root,
        vec!["targets/browser/runtime/src/lib.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(browser.browser_required);
    assert!(browser.workspace_shards["portable"]);
    assert!(!browser.esp32_required);
    assert!(!browser.conduitos_required);

    let kernel = plan_for_paths(
        &root,
        vec!["architecture/kernel/src/lib.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(!kernel.full_fallback);
    assert_eq!(kernel.esp32_targets.len(), ESP32_TARGETS.len());
    assert!(kernel.browser_required);
    assert_eq!(
        kernel.conduitos_x86_proofs.len(),
        CONDUITOS_X86_PROOFS.len()
    );
    assert_eq!(
        kernel.conduitos_architectures.len(),
        CONDUITOS_ARCHITECTURES.len()
    );
    assert!(kernel.conduitos_aarch64_product_required);

    let debugger_kernel = plan_for_paths(
        &root,
        DEBUGGER_KERNEL_SLICE
            .iter()
            .map(|path| (*path).to_owned())
            .collect(),
        &packages,
    )
    .unwrap();
    assert!(!debugger_kernel.full_fallback);
    assert!(!debugger_kernel.esp32_required);
    assert!(debugger_kernel.browser_required);
    assert!(!debugger_kernel.conduitos_required);

    let scheduler_alone = plan_for_paths(
        &root,
        vec!["architecture/kernel/src/scheduler.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(scheduler_alone.esp32_required);
    assert!(scheduler_alone.conduitos_required);

    let patchbay_package = plan_for_paths(
        &root,
        PATCHBAY_PACKAGE_SLICE
            .iter()
            .map(|path| (*path).to_owned())
            .collect(),
        &packages,
    )
    .unwrap();
    assert!(!patchbay_package.full_fallback);
    assert!(!patchbay_package.esp32_required);
    assert!(patchbay_package.browser_required);
    assert!(!patchbay_package.conduitos_required);

    let incomplete_patchbay_package =
        plan_for_paths(&root, vec!["Cargo.lock".to_owned()], &packages).unwrap();
    assert!(incomplete_patchbay_package.full_fallback);

    let manifest_backed_lock = plan_for_paths(
        &root,
        vec![
            "Cargo.lock".to_owned(),
            "products/patchbay/html/Cargo.toml".to_owned(),
            "products/patchbay/html/src/learned_demo.rs".to_owned(),
        ],
        &packages,
    )
    .unwrap();
    assert!(!manifest_backed_lock.full_fallback);
    assert!(manifest_backed_lock.browser_required);
    assert!(!manifest_backed_lock.esp32_required);
    assert!(!manifest_backed_lock.conduitos_required);

    let pi_zero_creche = plan_for_paths(
        &root,
        PI_ZERO_CRECHE_SLICE
            .iter()
            .map(|path| (*path).to_owned())
            .collect(),
        &packages,
    )
    .unwrap();
    assert!(!pi_zero_creche.full_fallback);
    assert!(!pi_zero_creche.esp32_required);
    assert!(pi_zero_creche.browser_required);
    assert!(!pi_zero_creche.conduitos_required);
    assert!(pi_zero_creche.workspace_shards["lint"]);
    assert!(pi_zero_creche.workspace_shards["test-products"]);

    let partial_pi_zero_creche = plan_for_paths(
        &root,
        vec![
            "scripts/ci/stage-creche-product.sh".to_owned(),
            "targets/raspberry-pi/fabrication-package/src/lib.rs".to_owned(),
        ],
        &packages,
    )
    .unwrap();
    assert!(!partial_pi_zero_creche.full_fallback);
    assert!(!partial_pi_zero_creche.esp32_required);
    assert!(partial_pi_zero_creche.browser_required);
    assert!(partial_pi_zero_creche.conduitos_required);
    assert!(partial_pi_zero_creche.pages_products_required);

    let shared_browser_presentation = plan_for_paths(
        &root,
        vec![
            "products/patchbay/html/assets/app.css".to_owned(),
            "products/patchbay/html/assets/app.js".to_owned(),
            "proof/browser/pages-front-door.spec.mjs".to_owned(),
            "scripts/ci/render-product-masthead.mjs".to_owned(),
            "scripts/ci/stage-book-product.sh".to_owned(),
            "scripts/ci/stage-creche-product.sh".to_owned(),
            "scripts/ci/stage-pages-root.sh".to_owned(),
            "scripts/ci/stage-patchbay-product.sh".to_owned(),
            "semantics/presentation/assets/product-masthead.mjs".to_owned(),
            "site/site.css".to_owned(),
            "targets/browser/host/assets/application-presentation.mjs".to_owned(),
        ],
        &packages,
    )
    .unwrap();
    assert!(!shared_browser_presentation.full_fallback);
    assert!(!shared_browser_presentation.esp32_required);
    assert!(shared_browser_presentation.browser_required);
    assert!(!shared_browser_presentation.conduitos_required);
    assert!(shared_browser_presentation.pages_products_required);
    assert!(shared_browser_presentation.workspace_shards["lint"]);

    let creche_presentation = plan_for_paths(
        &root,
        vec![
            "proof/browser/executable-book.spec.mjs".to_owned(),
            "scripts/ci/stage-creche-product.sh".to_owned(),
            "targets/browser/host/assets/creche-target-catalog.mjs".to_owned(),
            "targets/browser/host/assets/creche.css".to_owned(),
            "targets/browser/host/assets/creche.html".to_owned(),
            "targets/browser/host/assets/creche.mjs".to_owned(),
            "targets/browser/host/src/server.rs".to_owned(),
        ],
        &packages,
    )
    .unwrap();
    assert!(!creche_presentation.full_fallback);
    assert!(!creche_presentation.esp32_required);
    assert!(creche_presentation.browser_required);
    assert!(!creche_presentation.conduitos_required);
    assert!(creche_presentation.workspace_shards["lint"]);
    assert!(creche_presentation.workspace_shards["test-hosts"]);

    let unproved_creche_presentation = plan_for_paths(
        &root,
        vec!["targets/browser/host/assets/creche.mjs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(!unproved_creche_presentation.full_fallback);
    assert!(unproved_creche_presentation.browser_required);

    for path in ["Cargo.lock", ".github/workflows/check.yml"] {
        let global = plan_for_paths(&root, vec![path.to_owned()], &packages).unwrap();
        assert!(global.full_fallback, "{path}");
        assert!(global.workspace_shards.values().all(|required| *required));
    }

    for path in [
        ".github/workflows/book-pr-proof.yml",
        ".github/workflows/executable-book-pages.yml",
        ".github/workflows/patchbay-debugger-pr-proof.yml",
    ] {
        let focused_workflow = plan_for_paths(&root, vec![path.to_owned()], &packages).unwrap();
        assert!(!focused_workflow.full_fallback, "{path}");
        assert!(!focused_workflow.esp32_required, "{path}");
        assert!(focused_workflow.browser_required, "{path}");
        assert!(!focused_workflow.conduitos_required, "{path}");
        assert!(focused_workflow.workspace_shards["lint"], "{path}");
        assert!(
            focused_workflow
                .workspace_shards
                .iter()
                .filter(|(shard, _)| shard.as_str() != "lint")
                .all(|(_, required)| !required),
            "{path}"
        );
    }

    let repository_tool_test = plan_for_paths(
        &root,
        vec![
            "docs/proof-dependency-audit.toml".to_owned(),
            "docs/proof-dependency-boundary.md".to_owned(),
            "xtask/tests/proof_dependency_boundary.rs".to_owned(),
        ],
        &packages,
    )
    .unwrap();
    assert!(!repository_tool_test.full_fallback);
    assert!(!repository_tool_test.esp32_required);
    assert!(!repository_tool_test.browser_required);
    assert!(!repository_tool_test.conduitos_required);
    assert_eq!(repository_tool_test.changed_packages, ["xtask"]);
    assert!(repository_tool_test
        .affected_test_packages
        .contains(&"xtask".to_owned()));
    assert!(repository_tool_test.workspace_shards["lint"]);
    assert!(repository_tool_test.workspace_shards["test-products"]);
    for shard in ["test-foundation", "test-hosts", "portable", "pico"] {
        assert!(!repository_tool_test.workspace_shards[shard], "{shard}");
    }

    let pages_deploy_resolver = plan_for_paths(
        &root,
        PAGES_DEPLOY_RESOLVER_SLICE
            .iter()
            .map(|path| (*path).to_owned())
            .collect(),
        &packages,
    )
    .unwrap();
    assert!(!pages_deploy_resolver.full_fallback);
    assert!(!pages_deploy_resolver.browser_required);
    assert!(!pages_deploy_resolver.esp32_required);
    assert!(!pages_deploy_resolver.conduitos_required);
    assert!(pages_deploy_resolver.workspace_shards["lint"]);
    assert!(pages_deploy_resolver
        .workspace_shards
        .iter()
        .filter(|(shard, _)| shard.as_str() != "lint")
        .all(|(_, required)| !required));

    let unknown =
        plan_for_paths(&root, vec!["unknown/new-input.bin".to_owned()], &packages).unwrap();
    assert!(unknown.full_fallback);
}

#[test]
fn dev_dependencies_do_not_leak_conduitos_into_browser() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();
    let closure = dependency_closure(&packages, &suite_roots()["browser"]).unwrap();
    assert!(!closure.contains("conduitos"));
}

#[test]
fn changed_packages_select_their_reverse_dependent_test_shards() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();

    let app = plan_for_paths(&root, vec!["bodies/pete/src/lib.rs".to_owned()], &packages).unwrap();
    assert!(app.workspace_shards["test-products"]);
    assert!(!app.workspace_shards["test-hosts"]);
    assert!(!app.workspace_shards["portable"]);
    assert!(!app.workspace_shards["pico"]);
    assert!(!app.workspace_lint_full);
    assert_eq!(app.workspace_lint_packages, ["conduit-pete"]);
    assert!(app.affected_test_packages.contains(&"xtask".to_owned()));

    let kernel = plan_for_paths(
        &root,
        vec!["architecture/kernel/src/lib.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(kernel.workspace_shards["test-foundation"]);
    assert!(kernel.workspace_shards["test-hosts"]);
    assert!(kernel.workspace_shards["test-products"]);
    assert!(kernel.workspace_shards["portable"]);
    assert!(kernel.workspace_shards["pico"]);
    assert!(kernel
        .affected_test_packages
        .contains(&"conduit".to_owned()));

    let global = plan_for_paths(
        &root,
        vec![".github/workflows/check.yml".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(global.workspace_lint_full);
    assert!(global.workspace_lint_packages.is_empty());

    let time = plan_for_paths(
        &root,
        vec!["semantics/time/src/tick.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(time.workspace_shards["portable"]);
    assert!(time.workspace_shards["pico"]);

    let pico = plan_for_paths(
        &root,
        vec!["targets/rp2040/firmware/pico-w-signal/src/main.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(pico.workspace_shards["pico"]);
}

#[test]
fn esp32_paths_select_exact_target_obligations() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();

    let c3 = plan_for_paths(
        &root,
        vec!["targets/esp32/firmware/c3-signal/build.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert_eq!(c3.esp32_targets, ["c3"]);
    assert!(c3
        .affected_test_packages
        .contains(&"conduit-esp32-c3-signal".to_owned()));
    assert!(!c3
        .workspace_lint_packages
        .contains(&"conduit-esp32-c3-signal".to_owned()));

    let s3 = plan_for_paths(
        &root,
        vec!["targets/esp32/firmware/s3-signal/board-descriptor.json".to_owned()],
        &packages,
    )
    .unwrap();
    assert_eq!(s3.esp32_targets, ["s3"]);

    let shared_source = plan_for_paths(
        &root,
        vec!["targets/esp32/firmware/wroom-signal/src/main.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert_eq!(shared_source.esp32_targets.len(), 3);

    let shared_fabrication = plan_for_paths(
        &root,
        vec!["targets/esp32/fabrication/src/family.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert_eq!(shared_fabrication.esp32_targets.len(), 3);

    let shared_dependency = plan_for_paths(
        &root,
        vec!["architecture/kernel/src/lib.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert_eq!(shared_dependency.esp32_targets.len(), 3);
}

#[test]
fn conduitos_paths_select_exact_proof_obligations() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();

    let xhci = plan_for_paths(
        &root,
        vec!["targets/conduitos/src/arch/x86_64/xhci.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert_eq!(xhci.conduitos_x86_proofs.len(), 8);
    assert!(xhci.conduitos_x86_proofs.contains(&"xhci".to_owned()));
    assert!(xhci
        .conduitos_x86_proofs
        .contains(&"product-journey".to_owned()));
    assert!(xhci.conduitos_architectures.is_empty());
    assert!(!xhci.conduitos_aarch64_product_required);

    let riscv = plan_for_paths(
        &root,
        vec!["targets/conduitos/proof-appliances/riscv64/a3.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(riscv.conduitos_x86_proofs.is_empty());
    assert_eq!(riscv.conduitos_architectures, ["riscv64"]);
    assert!(!riscv.conduitos_aarch64_product_required);

    let product = plan_for_paths(
        &root,
        vec!["targets/conduitos/src/bin/aarch64_product.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(product.conduitos_x86_proofs.is_empty());
    assert!(product.conduitos_architectures.is_empty());
    assert!(product.conduitos_aarch64_product_required);

    let common = plan_for_paths(
        &root,
        vec!["targets/conduitos/src/composition.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert_eq!(common.conduitos_x86_proofs.len(), 8);
    assert_eq!(common.conduitos_architectures.len(), 4);
    assert!(common.conduitos_aarch64_product_required);
}

#[test]
fn workflow_validates_before_merge_without_a_post_merge_push_run() {
    let root = crate::workspace::workspace_root().unwrap();
    let workflow = fs::read_to_string(root.join(".github/workflows/check.yml")).unwrap();

    assert!(workflow.contains("on:\n  pull_request:\n  merge_group:\n"));
    assert!(!workflow.contains("\n  push:"));

    for output in ["esp32_required", "browser_required", "conduitos_required"] {
        assert!(workflow.contains(&format!(
            "{output}: ${{{{ steps.impact.outputs.{output} }}}}"
        )));
        assert!(workflow.contains(&format!(
            "github.event_name != 'pull_request' || needs.classify.result != 'success' || needs.classify.outputs.{output} == 'true'"
        )));
    }
    assert!(workflow.contains(
        "workspace_matrix: ${{ steps.impact.outputs.workspace_matrix || '[\"lint\",\"test-foundation\",\"test-hosts\",\"test-products\",\"portable\",\"pico\"]' }}"
    ));
    assert!(workflow.contains(
        "esp32_matrix: ${{ steps.impact.outputs.esp32_matrix || '[\"wroom\",\"c3\",\"s3\"]' }}"
    ));
    assert!(workflow.contains(
        "target: ${{ fromJSON(github.event_name == 'pull_request' && needs.classify.outputs.esp32_matrix"
    ));
    assert!(workflow.contains("name: esp32-firmware-${{ matrix.target }}"));
    assert!(workflow.contains(
        "conduitos_x86_matrix: ${{ steps.impact.outputs.conduitos_x86_matrix || '[\"kernel\",\"xhci\",\"usb\",\"hid\",\"keyboard\",\"front-door\",\"product-journey\",\"rescue\"]' }}"
    ));
    assert!(workflow.contains(
        "conduitos_architecture_matrix: ${{ steps.impact.outputs.conduitos_architecture_matrix || '[\"aarch64\",\"ia32\",\"riscv64\",\"loongarch64\"]' }}"
    ));
    assert!(workflow.contains("conduitos-proof-image:"));
    assert!(workflow.contains("cargo xtask conduitos prepare-proof-image --locked"));
    assert!(workflow.contains("xhci-proof --prepared-image --locked"));
    assert!(workflow.contains(
        "shard: ${{ fromJSON(github.event_name == 'pull_request' && needs.classify.outputs.workspace_matrix"
    ));
    assert!(workflow.contains(
        "CONDUIT_CI_LINT_FULL: ${{ github.event_name != 'pull_request' && 'true' || needs.classify.outputs.workspace_lint_full }}"
    ));
    assert!(workflow.contains(
        "CONDUIT_CI_LINT_PACKAGES: ${{ needs.classify.outputs.workspace_lint_packages }}"
    ));
    assert!(workflow.contains(
        "CONDUIT_CI_TEST_PACKAGES: ${{ needs.classify.outputs.workspace_test_packages }}"
    ));
    assert!(workflow.contains("\"${controller[@]}\" ci plan \"$BASE_SHA\" \"$HEAD_SHA\" --locked"));
    assert!(workflow.contains("name: Resolve the current trusted CI controller"));
    assert!(workflow.contains("git ls-remote --exit-code origin"));
    assert!(workflow.contains("git worktree add --detach \"$RUNNER_TEMP/conduit-ci-controller\""));
    assert!(workflow.contains("controller=(cargo xtask)"));
    assert!(workflow.contains(
        "controller=(\"$RUNNER_TEMP/conduit-ci-controller-target/debug/conduit-xtask-dispatch\")"
    ));
    assert!(workflow
        .contains("cargo build --manifest-path \"$RUNNER_TEMP/conduit-ci-controller/Cargo.toml\""));
    assert!(workflow.contains("--summary-out \"$GITHUB_STEP_SUMMARY\""));
    assert!(
        workflow.contains("comparison_base_sha: ${{ steps.changes.outputs.comparison_base_sha }}")
    );
    assert!(workflow.contains("BASE_SHA: ${{ steps.changes.outputs.comparison_base_sha }}"));
    assert!(workflow.contains("CONTROLLER_SHA: ${{ needs.classify.outputs.controller_sha }}"));
    assert!(workflow.contains("\"${controller[@]}\" ci standalone-locks --locked"));
    assert!(workflow.contains("name: ci-plan-${{ steps.changes.outputs.head_sha }}"));
}

#[test]
fn product_preflight_uses_the_trusted_controller_for_behind_candidates() {
    let root = crate::workspace::workspace_root().unwrap();
    let workflow =
        fs::read_to_string(root.join(".github/workflows/executable-book-pages.yml")).unwrap();
    assert!(workflow.contains("CONTROLLER_SHA: ${{ needs.plan.outputs.controller_sha }}"));
    assert!(workflow.contains(
        "\"$RUNNER_TEMP/conduit-ci-controller-target/debug/conduit-xtask-dispatch\"\n          ci standalone-locks --locked"
    ));
    assert!(!workflow.contains("run: cargo xtask ci standalone-locks --locked"));
}

#[test]
fn lightweight_dispatcher_owns_every_ci_identity_command() {
    let root = crate::workspace::workspace_root().unwrap();
    let manifest = fs::read_to_string(root.join("tools/xtask-dispatch/Cargo.toml")).unwrap();
    let source = fs::read_to_string(root.join("tools/xtask-dispatch/src/main.rs")).unwrap();
    let ci_source =
        fs::read_to_string(root.join("tools/xtask-dispatch/src/ci_dispatch.rs")).unwrap();

    assert!(manifest.contains("optional = true"));
    assert!(
        manifest.contains("clap = { version = \"4\", features = [\"derive\"], optional = true }")
    );
    assert!(source.contains("mod proof_graph;"));
    assert!(source.contains(".env(\"CARGO_TARGET_DIR\", \"target/xtask-host-release\")"));
    assert!(source.contains("std::env::remove_var(\"CARGO_TARGET_DIR\")"));
    for command in ["plan", "candidate", "reconcile", "attest-success"] {
        assert!(
            ci_source.contains(&format!("Some(\"{command}\")")),
            "missing lightweight dispatch for {command}"
        );
    }
}
