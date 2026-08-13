fn main() {
    use std::{env, fs, path::PathBuf};

    println!("cargo:rerun-if-changed=linker/x86_64.ld");
    println!("cargo:rerun-if-changed=linker/aarch64_a0.ld");
    println!("cargo:rerun-if-changed=linker/aarch64_a2.ld");
    println!("cargo:rerun-if-changed=linker/aarch64_a3.ld");
    println!("cargo:rerun-if-env-changed=CONDUITOS_BUILD_ID");
    println!("cargo:rerun-if-env-changed=CONDUITOS_IMAGE_ID");
    println!("cargo:rerun-if-env-changed=CONDUITOS_FABRICATION_RECORD");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"))
        .join("fabrication_record.rs");
    if let Some(source) = env::var_os("CONDUITOS_FABRICATION_RECORD") {
        println!(
            "cargo:rerun-if-changed={}",
            PathBuf::from(&source).display()
        );
        fs::copy(source, output).expect("copy generated fabrication record");
    } else {
        let build_id =
            env::var("CONDUITOS_BUILD_ID").unwrap_or_else(|_| "build:proof-appliance".into());
        let image_binding =
            env::var("CONDUITOS_IMAGE_ID").unwrap_or_else(|_| "image:proof-appliance".into());
        let proof_instrumentation = u16::from(env::var_os("CARGO_FEATURE_HOTPLUG_PROOF").is_some())
            | (u16::from(env::var_os("CARGO_FEATURE_SCRIPTED_KEYBOARD_PROOF").is_some()) << 1);
        fs::write(
            output,
            format!(
                "pub const EMBEDDED_FABRICATION: FabricationRecord = FabricationRecord {{ schema: FABRICATION_SCHEMA, profile_id: \"profile:proof-appliance\", build_id: {build_id:?}, image_binding: {image_binding:?}, target: \"conduitos/x86_64/pc\", implementations: ALL_KNOWN_IMPLEMENTATIONS, facilities: FACILITY_NATIVE_COMPOSITOR, resources: RESOURCE_PRESENTATION_SURFACE, bases: BASE_DISPLAY_SCANOUT, drivers: DRIVER_LINEAR_FRAMEBUFFER, presenters: PRESENTER_NATIVE_GRAPHICAL, proof_instrumentation: {proof_instrumentation}, runtime_arena_ceiling: 8388608, operation_slot_ceiling: 64, timer_slot_ceiling: 32, evidence_item_ceiling: 1024 }};\n"
            ),
        )
        .expect("write fallback fabrication record");
    }
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("Cargo sets manifest directory");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("x86_64")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none")
    {
        println!("cargo:rustc-link-arg=-T{manifest}/linker/x86_64.ld");
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none")
    {
        println!("cargo:rustc-link-arg-bin=conduitos-aarch64-a0=-T{manifest}/linker/aarch64_a0.ld");
        println!("cargo:rustc-link-arg-bin=conduitos-aarch64-a2=-T{manifest}/linker/aarch64_a2.ld");
        println!("cargo:rustc-link-arg-bin=conduitos-aarch64-a3=-T{manifest}/linker/aarch64_a3.ld");
    }
}
