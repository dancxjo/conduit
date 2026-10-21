//! Build the retained workspace Mask state as a separate immutable executable, never a Root library.
use std::{env, fs, path::PathBuf, process::Command};

use super::service_image_validation::{ServiceImage, ServiceProfile};

pub fn generate() {
    println!("cargo:rustc-check-cfg=cfg(conduitos_mask_image)");
    for source in [
        "mask-service",
        "mask-core",
        "src/service_memory.rs",
        "../../semantics/face",
        "../../semantics/human",
        "../../architecture/core",
        "../../Cargo.lock",
    ] {
        println!("cargo:rerun-if-changed={source}");
    }
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64")
        || env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none")
        || env::var_os("CARGO_FEATURE_NATIVE_COMPOSITOR").is_none()
    {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest"));
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output"));
    let target = env::var("TARGET").expect("Cargo target");
    let target_dir = output.join("mask-target");
    let flags = [
        "-C",
        "relocation-model=static",
        "-C",
        "code-model=kernel",
        "-C",
        "panic=abort",
        "--cfg",
        "curve25519_dalek_backend=\"serial\"",
    ];
    let result = Command::new(env::var_os("CARGO").expect("Cargo executable"))
        .args(["build", "--locked", "--release", "--manifest-path"])
        .arg(manifest.join("mask-service/Cargo.toml"))
        .args(["--target", &target, "--target-dir"])
        .arg(&target_dir)
        .env_remove("RUSTFLAGS")
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\u{1f}"))
        .output()
        .expect("start independent mask build");
    assert!(
        result.status.success(),
        "independent mask image failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let source = target_dir
        .join(&target)
        .join("release/conduitos-mask-service");
    let bytes = fs::read(&source).expect("read mask executable");
    // Exactly the mask profile: RX/RO image plus at most the first metadata
    // page as an initialized private RW/NX template. No writable image mapping.
    ServiceImage::for_profile(&bytes, ServiceProfile::Mask)
        .expect("validate bounded mask executable and private-data template");
    fs::copy(source, output.join("mask.elf")).expect("stage mask executable");
    println!("cargo:rustc-cfg=conduitos_mask_image");
}
