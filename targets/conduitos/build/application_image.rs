//! Build the canonical application models as a separate immutable executable, never a Root library.
use std::{env, fs, path::PathBuf, process::Command};

use super::service_image_validation::{ServiceImage, ServiceProfile};

pub fn generate() {
    println!("cargo:rustc-check-cfg=cfg(conduitos_application_image)");
    for source in [
        "application-service",
        "application-core",
        "src/service_memory.rs",
        "../../plots/tour",
        "../../plots/patchbay",
        "../../architecture",
        "../../semantics",
        "../../Cargo.lock",
    ] {
        println!("cargo:rerun-if-changed={source}");
    }
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64")
        || env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none")
        || (env::var_os("CARGO_FEATURE_NATIVE_COMPOSITOR").is_none()
            && env::var_os("CARGO_FEATURE_ORDINARY_DOMAIN_PROOF").is_none())
    {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest"));
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output"));
    let target = env::var("TARGET").expect("Cargo target");
    let target_dir = output.join("application-target");
    let flags = [
        "-C",
        "relocation-model=static",
        "-C",
        "code-model=kernel",
        "-C",
        "panic=abort",
    ];
    let result = Command::new(env::var_os("CARGO").expect("Cargo executable"))
        .args(["build", "--locked", "--release", "--manifest-path"])
        .arg(manifest.join("application-service/Cargo.toml"))
        .args(["--target", &target, "--target-dir"])
        .arg(&target_dir)
        .env_remove("RUSTFLAGS")
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\u{1f}"))
        .output()
        .expect("start independent application build");
    assert!(
        result.status.success(),
        "independent application image failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let source = target_dir
        .join(&target)
        .join("release/conduitos-application-service");
    let bytes = fs::read(&source).expect("read application executable");
    // Exactly the application profile: RX/RO image plus at most the first metadata
    // page as an initialized private RW/NX template. No writable image mapping.
    ServiceImage::for_profile(&bytes, ServiceProfile::Application)
        .expect("validate bounded application executable and private-data template");
    fs::copy(source, output.join("application.elf")).expect("stage application executable");
    println!("cargo:rustc-cfg=conduitos_application_image");
}
