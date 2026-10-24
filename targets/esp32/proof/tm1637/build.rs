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
        // Native task pools and Root statics must leave the CPU stack admitted.
        // Preserve the hardware guard as well; this floor is not a worst-case
        // stack proof for every future program or provider.
        let stack = out.join("native-stack.x");
        let floor = if std::env::var_os("CARGO_FEATURE_NATIVE_FACE_ROOT").is_some() {
            36 * 1024
        } else {
            28 * 1024
        };
        std::fs::write(&stack, format!(
            "ASSERT(_stack_start_cpu0 - _stack_end_cpu0 >= {floor}, \"native Root CPU stack below admitted profile floor\");\n"
        ))
        .unwrap();
        println!("cargo:rustc-link-arg=-T{}", stack.display());
        println!(
            "cargo:rustc-link-arg=-Wl,-Map={}",
            out.join("native-root.map").display()
        );
        build_owner_key::emit(&out);
        println!(
            "cargo:rerun-if-changed=../../../../bodies/environment/profiles/display-output.conduit"
        );
        println!("cargo:rerun-if-changed=../../../../plots/device-protocols/tm1637.conduit");
        std::fs::write(out.join("network-source.json"), &image).unwrap();
        std::fs::write(out.join("network-source.sha256"), Sha256::digest(&image)).unwrap();
        #[cfg(feature = "native-root")]
        {
            let display = conduit_environment_make::DisplayOutputSource::compile()
                .expect("checked display Source");
            let material = conduit_environment_make::DisplaySourceMaterial::prepare(&display)
                .expect("finite display software material");
            let mut output = String::new();
            material
                .write_rust(&mut output)
                .expect("resident display software module");
            std::fs::write(out.join("display-software.rs"), output).unwrap();
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
