//! Build the renderer as a separate immutable executable, never a Root library.
use std::{env, fs, path::PathBuf, process::Command};

use super::image_validation as image;

pub fn generate() {
    println!("cargo:rustc-check-cfg=cfg(conduitos_compositor_image)");
    for source in ["compositor-service", "compositor-core", "domain/memory.rs"] {
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
    let target_dir = output.join("compositor-target");
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
        .arg(manifest.join("compositor-service/Cargo.toml"))
        .args(["--target", &target, "--target-dir"])
        .arg(&target_dir)
        .env_remove("RUSTFLAGS")
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\u{1f}"))
        .output()
        .expect("start independent compositor build");
    assert!(
        result.status.success(),
        "independent compositor image failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let source = target_dir
        .join(&target)
        .join("release/conduitos-compositor-service");
    let bytes = fs::read(&source).expect("read compositor executable");
    // Runtime installation rechecks the full ServiceImage memory profile. This
    // build check also rejects writable/oversized executable segments.
    image::DomainImage::parse_in_window(&bytes, 62, 0x0040_0000, 2 * 1024 * 1024)
        .expect("validate immutable compositor executable");
    fs::copy(source, output.join("compositor.elf")).expect("stage compositor executable");
    println!("cargo:rustc-cfg=conduitos_compositor_image");
}
