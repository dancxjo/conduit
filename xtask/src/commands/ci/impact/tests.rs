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
