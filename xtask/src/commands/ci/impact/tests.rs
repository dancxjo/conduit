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
        (vec!["apps/pete/src/lib.rs"], (false, false, false)),
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
        vec!["apps/patchbay/model/src/lib.rs".to_owned()],
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

    for path in ["Cargo.lock", ".github/workflows/check.yml"] {
        let global = plan_for_paths(&root, vec![path.to_owned()], &packages).unwrap();
        assert!(global.full_fallback, "{path}");
        assert!(global.workspace_shards.values().all(|required| *required));
    }

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

    let app = plan_for_paths(&root, vec!["apps/pete/src/lib.rs".to_owned()], &packages).unwrap();
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
fn workflow_uses_the_plan_selectively_only_for_pull_requests() {
    let root = crate::workspace::workspace_root().unwrap();
    let workflow = fs::read_to_string(root.join(".github/workflows/check.yml")).unwrap();

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
    assert!(workflow.contains("cargo xtask ci plan \"$BASE_SHA\" \"$HEAD_SHA\" --locked"));
    assert!(workflow.contains("--summary-out \"$GITHUB_STEP_SUMMARY\""));
    assert!(workflow.contains("name: ci-impact-plan-${{ github.sha }}"));
}
