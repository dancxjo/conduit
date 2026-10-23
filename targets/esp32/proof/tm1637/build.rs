#[path = "../../../../bodies/environment/make/src/network_source.rs"]
mod network_source;
fn main() {
    println!("cargo:rustc-link-arg=-Tlinkall.x");
    println!("cargo:rerun-if-changed=../../../../bodies/environment/profiles/network.conduit");
    if std::env::var_os("CARGO_FEATURE_NATIVE_ROOT").is_some() {
        use sha2::{Digest, Sha256};
        let source = network_source::compile_network_setup().expect("checked network Source");
        let image = conduit_plot::flat_image::encode(&source).expect("finite flat Source image");
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        std::fs::write(out.join("network-source.json"), &image).unwrap();
        std::fs::write(out.join("network-source.sha256"), Sha256::digest(&image)).unwrap();
    }
}
