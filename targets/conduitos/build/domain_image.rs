//! Compile the pure implementation independently of the privileged image.
use std::{env, path::PathBuf, process::Command};
#[allow(dead_code)]
#[path = "../src/domain_image.rs"]
mod image;

pub fn generate() {
    for source in [
        "domain/main.rs",
        "domain/build.rs",
        "domain/Cargo.toml",
        "domain/Cargo.lock",
        "domain/allocation.rs",
        "domain/keymap.rs",
        "domain/morse.rs",
        "domain/timer.rs",
        "src/tour_timer_runtime.rs",
        "src/tour_timer_runtime",
        "../../architecture/kernel",
        "../../semantics/time",
        "../../semantics/text",
        "../../semantics/catalog/src/text_state/retained.rs",
        "../../semantics/human",
        "../../architecture/core",
        "../../architecture/assigned-plan",
        "../../architecture/plot",
        "domain/gate.rs",
        "domain/memory.rs",
        "domain/frame.rs",
        "domain/layout.rs",
        "domain/probes.rs",
        "domain/probes_ia32.rs",
        "domain/probes_aarch64.rs",
        "domain/probes_riscv64.rs",
        "domain/probes_loongarch64.rs",
        "domain/probe_gate.rs",
        "domain/linker.ld",
        "src/text_transform.rs",
    ] {
        println!("cargo:rerun-if-changed={source}");
    }
    let architecture = env::var("CARGO_CFG_TARGET_ARCH").expect("Cargo sets architecture");
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("Cargo sets target OS");
    println!("cargo:rustc-check-cfg=cfg(conduitos_protected_execution)");
    println!("cargo:rustc-check-cfg=cfg(conduitos_domain_image)");
    println!("cargo:rustc-check-cfg=cfg(conduitos_domain_backend)");
    let protected = match architecture.as_str() {
        "x86_64" => target_os == "none",
        "x86" => target_os == "linux" && env::var_os("CARGO_FEATURE_IA32_PRODUCT").is_some(),
        "aarch64" => {
            target_os == "none"
                && env::var_os("CARGO_FEATURE_AARCH64_PRODUCT").is_some()
                && env::var_os("CARGO_FEATURE_AARCH64_ORANGE_PI_5").is_none()
        }
        "riscv64" => target_os == "none" && env::var_os("CARGO_FEATURE_RISCV64_PRODUCT").is_some(),
        "loongarch64" => {
            target_os == "none" && env::var_os("CARGO_FEATURE_LOONGARCH64_PRODUCT").is_some()
        }
        _ => false,
    };
    if protected {
        println!("cargo:rustc-cfg=conduitos_protected_execution");
    }
    if (target_os != "none" && !(architecture == "x86" && target_os == "linux"))
        || !matches!(
            architecture.as_str(),
            "x86_64" | "x86" | "aarch64" | "riscv64" | "loongarch64"
        )
    {
        return;
    }
    println!("cargo:rustc-cfg=conduitos_domain_backend");
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets manifest"));
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets output"));
    let target = env::var("TARGET").expect("Cargo sets target");
    let mut flags = vec![
        "-C".to_string(),
        "panic=abort".into(),
        "-C".into(),
        "relocation-model=static".into(),
        "-C".into(),
        "code-model=small".into(),
        "-C".into(),
        format!("link-arg=-T{}", manifest.join("domain/linker.ld").display()),
        "-C".into(),
        "link-arg=--build-id=none".into(),
        "-C".into(),
        "link-arg=-z".into(),
        "-C".into(),
        "link-arg=max-page-size=4096".into(),
    ];
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
        flags.extend([
            "-C".into(),
            format!(
                "link-arg=--defsym=__domain_text_start={:#x}",
                image::IA32_USER_TEXT_START
            ),
            "-C".into(),
            format!("linker={}", linker.display()),
            "-C".into(),
            "linker-flavor=ld.lld".into(),
            "-C".into(),
            "link-arg=--nostdlib".into(),
            "-C".into(),
            "link-arg=-no-pie".into(),
        ]);
    }
    let target_dir = output.join("domain-target");
    let mut compiler = Command::new(env::var_os("CARGO").expect("Cargo sets Cargo"));
    compiler
        .args(["build", "--locked", "--release", "--manifest-path"])
        .arg(manifest.join("domain/Cargo.toml"))
        .args(["--target", &target, "--target-dir"])
        .arg(&target_dir)
        .env_remove("RUSTFLAGS")
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\u{1f}"));
    if matches!(
        architecture.as_str(),
        "x86_64" | "x86" | "aarch64" | "riscv64" | "loongarch64"
    ) && env::var_os("CARGO_FEATURE_ORDINARY_DOMAIN_PROOF").is_some()
    {
        compiler.args(["--features", "proof"]);
    }
    let result = compiler.output().expect("start independent domain build");
    if !result.status.success() {
        panic!(
            "pure domain image failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    std::fs::copy(
        target_dir.join(&target).join("release/conduitos-domain"),
        output.join("domain.elf"),
    )
    .expect("stage independently linked domain");
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
