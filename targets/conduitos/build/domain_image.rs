//! Compile the pure implementation independently of the privileged image.
use std::{env, path::PathBuf, process::Command};
#[allow(dead_code)]
#[path = "../src/domain_image.rs"]
mod image;

pub fn generate() {
    for source in [
        "domain/main.rs",
        "domain/gate.rs",
        "domain/memory.rs",
        "domain/frame.rs",
        "domain/probes.rs",
        "domain/linker.ld",
        "src/text_transform.rs",
    ] {
        println!("cargo:rerun-if-changed={source}");
    }
    let architecture = env::var("CARGO_CFG_TARGET_ARCH").expect("Cargo sets architecture");
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("Cargo sets target OS");
    if (target_os != "none" && !(architecture == "x86" && target_os == "linux"))
        || !matches!(
            architecture.as_str(),
            "x86_64" | "x86" | "aarch64" | "riscv64" | "loongarch64"
        )
    {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets manifest"));
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets output"));
    let mut compiler = Command::new(env::var_os("RUSTC").expect("Cargo sets rustc"));
    compiler
        .args(["--edition=2024", "--target"])
        .arg(env::var_os("TARGET").expect("Cargo sets target"))
        .args([
            "-D",
            "warnings",
            "-C",
            "opt-level=2",
            "-C",
            "panic=abort",
            "-C",
            "relocation-model=static",
            "-C",
            "code-model=small",
            "-C",
        ])
        .arg(format!(
            "link-arg=-T{}",
            manifest.join("domain/linker.ld").display()
        ))
        .args([
            "-C",
            "link-arg=--build-id=none",
            "-C",
            "link-arg=-z",
            "-C",
            "link-arg=max-page-size=4096",
        ])
        .arg(manifest.join("domain/main.rs"))
        .arg("-o")
        .arg(output.join("domain.elf"));
    compiler.args(["--check-cfg", "cfg(domain_proof)"]);
    if architecture == "x86_64" && env::var_os("CARGO_FEATURE_ORDINARY_DOMAIN_PROOF").is_some() {
        compiler.args(["--cfg", "domain_proof"]);
    }
    if architecture == "x86" {
        let sysroot = Command::new(env::var_os("RUSTC").expect("Cargo sets rustc"))
            .args(["--print", "sysroot"])
            .output()
            .expect("locate bundled linker");
        assert!(sysroot.status.success(), "rustc sysroot query failed");
        let linker = PathBuf::from(
            String::from_utf8(sysroot.stdout)
                .expect("UTF-8 sysroot")
                .trim(),
        )
        .join("lib/rustlib")
        .join(env::var("HOST").expect("Cargo sets host"))
        .join("bin/rust-lld");
        compiler
            .arg("-C")
            .arg(format!("linker={}", linker.display()))
            .args([
                "-C",
                "linker-flavor=ld.lld",
                "-C",
                "link-arg=--nostdlib",
                "-C",
                "link-arg=-no-pie",
            ]);
    }
    let result = compiler.output().expect("start domain image compiler");
    if !result.status.success() {
        panic!(
            "pure domain image failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let machine = match architecture.as_str() {
        "x86" => 3,
        "x86_64" => 62,
        "aarch64" => 183,
        "riscv64" => 243,
        "loongarch64" => 258,
        _ => return,
    };
    let bytes = std::fs::read(output.join("domain.elf")).expect("read compiled domain");
    image::DomainImage::parse(&bytes, machine).expect("validate immutable domain image");
}
