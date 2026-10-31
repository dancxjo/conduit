//! Checked Make material only; no current Host, Boot, radio or Plan is linked.
pub fn emit(out: &std::path::Path) {
    use sha2::{Digest, Sha256};
    println!("cargo:rerun-if-changed=../../../../bodies/environment/profiles/network-scan.conduit");
    let source = conduit_environment_make::compile_network_scan().expect("checked scan Source");
    let material = conduit_environment_make::NetworkSourceMaterial::prepare(&source)
        .expect("finite scan Source material");
    let mut rust = String::new();
    material
        .write_rust(&mut rust)
        .expect("linked scan Source facts");
    std::fs::write(out.join("scan-software.rs"), rust).unwrap();
    std::fs::write(out.join("scan-source.bin"), material.planning_image()).unwrap();
    std::fs::write(
        out.join("scan-source.sha256"),
        Sha256::digest(material.planning_image()),
    )
    .unwrap();
}
