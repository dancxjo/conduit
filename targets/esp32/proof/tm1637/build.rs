mod build_owner_key;
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
        build_owner_key::emit(&out);
        std::fs::write(out.join("network-source.json"), &image).unwrap();
        std::fs::write(out.join("network-source.sha256"), Sha256::digest(&image)).unwrap();
        #[cfg(feature = "native-root")]
        {
            let source = conduit_environment_make::compile_appliance_source()
                .expect("checked environmental appliance Source");
            let recipe = conduit_environment_make::NativeSoftwareSourceRecipe::prepare(&source)
                .expect("checked appliance software construction");
            let mut rust = String::new();
            recipe
                .write_source_rust(&mut rust)
                .expect("resident Source module");
            std::fs::write(out.join("appliance-software.rs"), rust).unwrap();
        }
    }
}
