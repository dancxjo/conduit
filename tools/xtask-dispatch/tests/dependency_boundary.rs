//! A direct-dependency allowlist alone missed the Host make transitive graph.
#[test]
fn default_dispatcher_has_no_transitive_product_or_semantic_packages() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let output = std::process::Command::new("cargo")
        .args([
            "tree",
            "--locked",
            "-p",
            "conduit-xtask-dispatch",
            "--edges",
            "normal,build",
            "--prefix",
            "none",
        ])
        .current_dir(root)
        .output()
        .expect("inspect actual default Cargo dependency graph");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph = String::from_utf8(output.stdout).unwrap();
    let workspace_packages = graph
        .lines()
        .filter(|line| line.starts_with("conduit-") || line.starts_with("xtask "))
        .collect::<Vec<_>>();
    assert_eq!(
        workspace_packages.len(),
        1,
        "default bootstrap pulled workspace machinery:\n{graph}"
    );
    assert!(workspace_packages[0].starts_with("conduit-xtask-dispatch "));
}
