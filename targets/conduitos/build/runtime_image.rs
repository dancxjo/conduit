//! Build the canonical runtime as a separate immutable executable, never a Root library.
use std::{env, fs, path::PathBuf, process::Command};

use super::service_image_validation::{ServiceImage, ServiceProfile};

pub fn generate() {
    println!("cargo:rustc-check-cfg=cfg(conduitos_runtime_image)");
    for source in [
        "runtime-service",
        "runtime-core",
        "src/service_memory.rs",
        "../../architecture/kernel",
        "../../architecture/assigned-plan",
        "../../Cargo.lock",
    ] {
        println!("cargo:rerun-if-changed={source}");
    }
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64")
        || env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none")
    {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest"));
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output"));
    let target = env::var("TARGET").expect("Cargo target");
    let target_dir = output.join("runtime-target");
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
        .arg(manifest.join("runtime-service/Cargo.toml"))
        .args(["--target", &target, "--target-dir"])
        .arg(&target_dir)
        .env_remove("RUSTFLAGS")
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\u{1f}"))
        .output()
        .expect("start independent runtime build");
    assert!(
        result.status.success(),
        "independent runtime image failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let source = target_dir
        .join(&target)
        .join("release/conduitos-runtime-service");
    let bytes = fs::read(&source).expect("read runtime executable");
    // Exactly the runtime profile: RX/RO image plus at most the first metadata
    // page as an initialized private RW/NX template. No writable image mapping.
    ServiceImage::for_profile(&bytes, ServiceProfile::Runtime)
        .expect("validate bounded runtime executable and private-data template");
    ServiceImage::for_profile(&bytes, ServiceProfile::OrdinaryRuntime)
        .expect("validate independent ordinary runtime image");
    fs::copy(source, output.join("runtime.elf")).expect("stage runtime executable");
    println!("cargo:rustc-cfg=conduitos_runtime_image");
}
