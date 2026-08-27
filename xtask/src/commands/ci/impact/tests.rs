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
            vec!["firmware/conduit-esp32-c3-signal/src/main.rs"],
            (true, false, false),
        ),
        (vec!["proof/browser/pointer.spec.mjs"], (false, true, false)),
        (vec!["hosts/conduitos/src/main.rs"], (false, false, true)),
        (vec!["crates/conduit-kernel/src/lib.rs"], (true, true, true)),
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
        assert_eq!(plan.esp32_required, !plan.esp32_targets.is_empty());
    }
}

#[test]
fn esp32_targets_and_portable_shards_follow_exact_dependencies() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover(&root).unwrap();

    let c3 = plan_for_paths(
        &root,
        vec!["firmware/conduit-esp32-c3-signal/src/main.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert_eq!(c3.esp32_targets, ["c3"]);

    let family = plan_for_paths(
        &root,
        vec!["targets/esp32/fabrication/src/lib.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert_eq!(family.esp32_targets, ["c3", "s3", "wroom"]);

    let pete = plan_for_paths(&root, vec!["apps/pete/src/lib.rs".to_owned()], &packages).unwrap();
    assert!(!pete.workspace_shards["portable"]);
    assert!(!pete.workspace_shards["pico"]);

    let kernel = plan_for_paths(
        &root,
        vec!["crates/conduit-kernel/src/lib.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(kernel.workspace_shards["portable"]);
    assert!(kernel.workspace_shards["pico"]);
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

    let kernel = plan_for_paths(
        &root,
        vec!["crates/conduit-kernel/src/lib.rs".to_owned()],
        &packages,
    )
    .unwrap();
    assert!(kernel.workspace_shards["test-foundation"]);
    assert!(kernel.workspace_shards["test-hosts"]);
    assert!(kernel.workspace_shards["test-products"]);
    assert!(kernel
        .affected_test_packages
        .contains(&"conduit".to_owned()));
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
        "target: ${{ fromJSON(github.event_name == 'pull_request' && needs.classify.outputs.esp32_required == 'true' && needs.classify.outputs.esp32_matrix"
    ));
    assert!(workflow.contains(
        "shard: ${{ fromJSON(github.event_name == 'pull_request' && needs.classify.outputs.workspace_matrix"
    ));
    assert!(workflow.contains("cargo xtask ci plan \"$BASE_SHA\" \"$HEAD_SHA\" --locked"));
    assert!(workflow.contains("--summary-out \"$GITHUB_STEP_SUMMARY\""));
    assert!(workflow.contains("name: ci-impact-plan-${{ github.sha }}"));
}
