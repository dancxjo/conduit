fn main() {
    use std::{env, fs, path::PathBuf};

    println!("cargo:rerun-if-changed=firmware/linker/x86_64.ld");
    println!("cargo:rerun-if-changed=proof/appliances/aarch64/linker/a0.ld");
    println!("cargo:rerun-if-changed=proof/appliances/aarch64/linker/a2.ld");
    println!("cargo:rerun-if-changed=proof/appliances/aarch64/linker/a3.ld");
    println!("cargo:rerun-if-changed=firmware/linker/ia32_product.ld");
    println!("cargo:rerun-if-changed=firmware/linker/aarch64_product.ld");
    println!("cargo:rerun-if-changed=firmware/linker/riscv64_product.ld");
    println!("cargo:rerun-if-changed=firmware/linker/loongarch64_product.ld");
    println!("cargo:rerun-if-changed=firmware/linker/aarch64_orange_pi_5.ld");
    println!("cargo:rerun-if-changed=proof/appliances/armv6-rpi-b-plus/linker/a0.ld");
    println!("cargo:rerun-if-changed=proof/appliances/armv6-rpi-b-plus/linker/a2.ld");
    println!("cargo:rerun-if-changed=proof/appliances/armv6-rpi-b-plus/linker/a3.ld");
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
        if env::var_os("CARGO_FEATURE_AARCH64_ORANGE_PI_5").is_some() {
            fs::write(
                output,
                format!(
                    "pub const EMBEDDED_FABRICATION: FabricationRecord = FabricationRecord {{ schema: FABRICATION_SCHEMA, profile_id: \"profile:conduitos-orange-pi-5-rk3588s-v1\", build_id: {build_id:?}, image_binding: {image_binding:?}, target: \"conduitos/aarch64/orange-pi-5-rk3588s\", implementations: IMPL_TIME_TICK | IMPL_TICK_PRESENTATION | IMPL_TEXT_LITERAL | IMPL_TEXT_UPPER | IMPL_TEXT_PRESENTATION | IMPL_LINEAR_PRESENTER, facilities: 0, resources: 0, bases: BASE_SERIAL_TEXT, drivers: DRIVER_DW_APB_UART2, presenters: PRESENTER_LINEAR_SERIAL, proof_instrumentation: 0, presentation_surface_slots: 0, presentation_surface_bytes: 0, runtime_arena_ceiling: 8388608, operation_slot_ceiling: 64, timer_slot_ceiling: 32, evidence_item_ceiling: 1024 }};\n"
                ),
            )
            .expect("write Orange Pi 5 fabrication record");
        } else {
            let proof_instrumentation =
                u16::from(env::var_os("CARGO_FEATURE_HOTPLUG_PROOF").is_some())
                    | (u16::from(env::var_os("CARGO_FEATURE_SCRIPTED_KEYBOARD_PROOF").is_some())
                        << 1);
            fs::write(
                output,
                format!(
                    "pub const EMBEDDED_FABRICATION: FabricationRecord = FabricationRecord {{ schema: FABRICATION_SCHEMA, profile_id: \"profile:proof-appliance\", build_id: {build_id:?}, image_binding: {image_binding:?}, target: \"conduitos/x86_64/pc\", implementations: ALL_KNOWN_IMPLEMENTATIONS & !IMPL_LINEAR_PRESENTER & !IMPL_HTTP_CLIENT, facilities: FACILITY_NATIVE_COMPOSITOR, resources: RESOURCE_PRESENTATION_SURFACE, bases: BASE_DISPLAY_SCANOUT, drivers: DRIVER_LINEAR_FRAMEBUFFER, presenters: PRESENTER_NATIVE_GRAPHICAL, proof_instrumentation: {proof_instrumentation}, presentation_surface_slots: 2, presentation_surface_bytes: 4194304, runtime_arena_ceiling: 8388608, operation_slot_ceiling: 64, timer_slot_ceiling: 32, evidence_item_ceiling: 1024 }};\n"
                ),
            )
            .expect("write fallback fabrication record");
        }
    }
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("Cargo sets manifest directory");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("x86_64")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none")
    {
        println!("cargo:rustc-link-arg=-T{manifest}/firmware/linker/x86_64.ld");
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none")
    {
        println!(
            "cargo:rustc-link-arg-bin=conduitos-aarch64-a0=-T{manifest}/proof/appliances/aarch64/linker/a0.ld"
        );
        println!(
            "cargo:rustc-link-arg-bin=conduitos-aarch64-a2=-T{manifest}/proof/appliances/aarch64/linker/a2.ld"
        );
        println!(
            "cargo:rustc-link-arg-bin=conduitos-aarch64-a3=-T{manifest}/proof/appliances/aarch64/linker/a3.ld"
        );
        println!(
            "cargo:rustc-link-arg-bin=conduitos-aarch64-product=-T{manifest}/firmware/linker/aarch64_product.ld"
        );
        println!(
            "cargo:rustc-link-arg-bin=conduitos-aarch64-orange-pi-5=-T{manifest}/firmware/linker/aarch64_orange_pi_5.ld"
        );
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("x86")
        && std::env::var_os("CARGO_FEATURE_IA32_PRODUCT").is_some()
    {
        println!(
            "cargo:rustc-link-arg-bin=conduitos-ia32-product=-T{manifest}/firmware/linker/ia32_product.ld"
        );
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("riscv64")
        && std::env::var_os("CARGO_FEATURE_RISCV64_PRODUCT").is_some()
    {
        println!(
            "cargo:rustc-link-arg-bin=conduitos-riscv64-product=-T{manifest}/firmware/linker/riscv64_product.ld"
        );
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("loongarch64")
        && std::env::var_os("CARGO_FEATURE_LOONGARCH64_PRODUCT").is_some()
    {
        println!(
            "cargo:rustc-link-arg-bin=conduitos-loongarch64-product=-T{manifest}/firmware/linker/loongarch64_product.ld"
        );
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("arm")
        && std::env::var_os("CARGO_FEATURE_ARMV6_RPI_B_PLUS_A0").is_some()
    {
        println!(
            "cargo:rustc-link-arg-bin=conduitos-armv6-rpi-b-plus-a0=-T{manifest}/proof/appliances/armv6-rpi-b-plus/linker/a0.ld"
        );
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("arm")
        && std::env::var_os("CARGO_FEATURE_ARMV6_RPI_B_PLUS_A2").is_some()
    {
        println!(
            "cargo:rustc-link-arg-bin=conduitos-armv6-rpi-b-plus-a2=-T{manifest}/proof/appliances/armv6-rpi-b-plus/linker/a2.ld"
        );
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("arm")
        && std::env::var_os("CARGO_FEATURE_ARMV6_RPI_B_PLUS_A3").is_some()
    {
        println!(
            "cargo:rustc-link-arg-bin=conduitos-armv6-rpi-b-plus-a3=-T{manifest}/proof/appliances/armv6-rpi-b-plus/linker/a3.ld"
        );
    }
}
