//! Build the bounded network protocol state as a separate immutable executable, never a Root library.
use std::{env, fs, path::PathBuf, process::Command};

use super::service_image_validation::{ServiceImage, ServiceProfile};

pub fn generate() {
    println!("cargo:rustc-check-cfg=cfg(conduitos_network_image)");
    for source in [
        "network-service",
        "network-core",
        "src/service_memory.rs",
        "../../semantics/net",
        "../../Cargo.lock",
    ] {
        println!("cargo:rerun-if-changed={source}");
    }
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64")
        || env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none")
        || env::var_os("CARGO_FEATURE_NATIVE_OWNER_NETWORK").is_none()
    {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest"));
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output"));
    let target = env::var("TARGET").expect("Cargo target");
    let target_dir = output.join("network-target");
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
        .arg(manifest.join("network-service/Cargo.toml"))
        .args(["--target", &target, "--target-dir"])
        .arg(&target_dir)
        .env_remove("RUSTFLAGS")
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\u{1f}"))
        .output()
        .expect("start independent network build");
    assert!(
        result.status.success(),
        "independent network image failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let source = target_dir
        .join(&target)
        .join("release/conduitos-network-service");
    let bytes = fs::read(&source).expect("read network executable");
    // Exactly the network profile: RX/RO image plus at most the first metadata
    // page as an initialized private RW/NX template. No writable image mapping.
    ServiceImage::for_profile(&bytes, ServiceProfile::Network)
        .expect("validate bounded network executable and private-data template");
    fs::copy(source, output.join("network.elf")).expect("stage network executable");
    println!("cargo:rustc-cfg=conduitos_network_image");
}
