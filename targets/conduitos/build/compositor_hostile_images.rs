//! Build distinct hostile fixtures only for the architecture proof appliance.
use std::{env, fs, path::PathBuf, process::Command};

#[allow(dead_code)]
#[path = "../src/domain_image.rs"]
mod image;

pub fn generate() {
    println!("cargo:rustc-check-cfg=cfg(conduitos_compositor_hostile_images)");
    for source in [
        "compositor-hostile-proof",
        "compositor-core",
        "domain/memory.rs",
        "domain/memory_words.rs",
    ] {
        println!("cargo:rerun-if-changed={source}");
    }
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64")
        || env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none")
        || env::var_os("CARGO_FEATURE_ORDINARY_DOMAIN_PROOF").is_none()
    {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest"));
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output"));
    let target = env::var("TARGET").expect("Cargo target");
    let target_dir = output.join("compositor-hostile-target");
    let flags = [
        "-C",
        "relocation-model=static",
        "-C",
        "code-model=kernel",
        "-C",
        "panic=abort",
    ];
    let result = Command::new(env::var_os("CARGO").expect("Cargo executable"))
        .args([
            "build",
            "--locked",
            "--release",
            "--bins",
            "--manifest-path",
        ])
        .arg(manifest.join("compositor-hostile-proof/Cargo.toml"))
        .args(["--target", &target, "--target-dir"])
        .arg(&target_dir)
        .env_remove("RUSTFLAGS")
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\u{1f}"))
        .output()
        .expect("start hostile proof build");
    assert!(
        result.status.success(),
        "hostile proof image failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    for name in ["envelope", "shape", "fault", "loop"] {
        let source = target_dir
            .join(&target)
            .join(format!("release/compositor-proof-{name}"));
        let bytes = fs::read(&source).expect("read hostile proof executable");
        image::DomainImage::parse_in_window(&bytes, 62, 0x0040_0000, 2 * 1024 * 1024)
            .expect("validate immutable hostile proof executable");
        fs::copy(source, output.join(format!("compositor-proof-{name}.elf")))
            .expect("stage hostile proof executable");
    }
    println!("cargo:rustc-cfg=conduitos_compositor_hostile_images");
}
