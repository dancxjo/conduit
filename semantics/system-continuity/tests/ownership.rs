use std::path::Path;

#[test]
fn semantic_owner_has_no_r1_composition_or_proof_dependency() {
    let manifest = include_str!("../Cargo.toml");
    let production = manifest
        .split_once("[dev-dependencies]")
        .map_or(manifest, |(production, _)| production);
    assert!(!production.contains("proof/"));
    assert!(!production.contains("conduit-r1-network-conformance"));
    assert!(!production.contains("r1-recovery"));

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for retired in [
        "r1_control_planning.rs",
        "r1_host_loss.rs",
        "r1_planning.rs",
        "r1_recovery.rs",
    ] {
        assert!(
            !source.join(retired).exists(),
            "exact R1 proof composition returned to semantic ownership: {retired}"
        );
    }
}
