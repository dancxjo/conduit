//! Publication invariants are behavioral tests, independent of YAML job names.
#[test]
fn pipeline_preserves_target_and_artifact_identity() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = std::fs::read_dir(root.join("proof/ci"))
        .expect("proof directory")
        .map(|entry| entry.expect("proof entry").path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with("pipeline-") && name.ends_with(".spec.mjs")
        })
        .collect::<Vec<_>>();
    assert!(!files.is_empty(), "pipeline invariant tests must exist");
    assert!(std::process::Command::new("node")
        .arg("--test")
        .args(files)
        .current_dir(root)
        .status()
        .expect("Node.js is required for pipeline proof")
        .success());
}
