use super::*;

#[test]
fn representative_changes_select_only_owned_heavy_suites() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover_packages(&root).unwrap();
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
    }
}

#[test]
fn dev_dependencies_do_not_leak_conduitos_into_browser() {
    let root = crate::workspace::workspace_root().unwrap();
    let packages = discover_packages(&root).unwrap();
    let closure = dependency_closure(&packages, &suite_roots()["browser"]).unwrap();
    assert!(!closure.contains("conduitos"));
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
    assert!(workflow.contains("cargo xtask ci plan \"$BASE_SHA\" \"$HEAD_SHA\" --locked"));
    assert!(workflow.contains("--summary-out \"$GITHUB_STEP_SUMMARY\""));
    assert!(workflow.contains("name: ci-impact-plan-${{ github.sha }}"));
}
