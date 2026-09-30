use std::{fs, process::Command};

use super::{
    ia32_a0,
    profile::{Paths, IA32_OBJECT_TARGET},
    report::{git_head, sha256_file, ArtifactRole, BuildRecord},
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;

const BINARY: &str = "conduitos-ia32-a3";

pub fn execute(opts: &GlobalOpts) -> Result<BuildRecord, ConduitosError> {
    let paths = Paths::new(ConduitosArch::Ia32)?;
    if opts.dry_run {
        println!("cargo build -p conduitos --bin {BINARY} --features ia32-a3 --target {IA32_OBJECT_TARGET} --release");
        return record(&paths, "dry-run".into());
    }
    let _ = ia32_a0::execute(opts)?;
    fs::create_dir_all(&paths.target)
        .map_err(|error| refusal("build-output-unavailable", error.to_string()))?;
    let commit = git_head(&paths.root)?;
    let linker = ia32_a0::rust_lld(&paths.root)?;
    let script = paths
        .root
        .join("targets/conduitos/proof/appliances/ia32/linker/a3.ld");
    let rustflags = format!(
        "-C relocation-model=static -C panic=abort -C linker={} -C link-arg=-T{} -C link-arg=--nostdlib",
        linker.display(), script.display()
    );
    let mut command = Command::new("cargo");
    command
        .args([
            "build",
            "-p",
            "conduitos",
            "--bin",
            BINARY,
            "--features",
            "ia32-a3",
            "--target",
            IA32_OBJECT_TARGET,
            "--release",
        ])
        .current_dir(&paths.root)
        .env("RUSTFLAGS", rustflags)
        .env("CONDUITOS_BUILD_ID", &commit)
        .env(
            "CONDUITOS_IMAGE_ID",
            format!("conduitos-image/{commit}/ia32/v1"),
        );
    if opts.locked {
        command.arg("--locked");
    }
    let status = command
        .status()
        .map_err(|error| refusal("ia32-a3-toolchain-unavailable", error.to_string()))?;
    if !status.success() {
        return Err(refusal("ia32-a3-compile-link-failed", status.to_string()));
    }
    let built = paths
        .root
        .join(format!("target/{IA32_OBJECT_TARGET}/release/{BINARY}"));
    fs::copy(built, &paths.kernel)
        .map_err(|error| refusal("build-output-unavailable", error.to_string()))?;
    let bytes = fs::read(&paths.kernel)
        .map_err(|error| refusal("artifact-unavailable", error.to_string()))?;
    ia32_a0::inspect_elf(&bytes)?;
    let symbols = super::profile::command(
        "readelf",
        &["-sW", paths.kernel.to_str().unwrap()],
        &paths.root,
        "readelf-unavailable",
    )?;
    let symbols = String::from_utf8_lossy(&symbols.stdout);
    if !symbols
        .lines()
        .any(|line| line.contains("GLOBAL") && line.ends_with("conduitos_ia32_a3_start"))
        || symbols.contains("x86_64")
        || symbols.contains("aarch64")
    {
        return Err(refusal(
            "invalid-ia32-a3-artifact",
            "exact IA-32 A3 entry absent or architecture alias leaked",
        ));
    }
    let digest = sha256_file(&paths.kernel)?;
    let record = record(&paths, digest)?;
    fs::write(
        paths.target.join("build.json"),
        serde_json::to_vec_pretty(&record)
            .map_err(|e| refusal("build-record-failed", e.to_string()))?,
    )
    .map_err(|e| refusal("build-record-failed", e.to_string()))?;
    if !opts.quiet && !opts.json {
        println!("ConduitOS IA-32 A3 ELF: {}", paths.kernel.display());
    }
    Ok(record)
}

fn record(paths: &Paths, digest: String) -> Result<BuildRecord, ConduitosError> {
    Ok(BuildRecord {
        schema: "conduit.conduitos.build/v2",
        artifact_role: ArtifactRole::ArchitectureProofAppliance,
        base_commit: git_head(&paths.root)?,
        architecture: "ia32",
        rust_target: "i686-freestanding-elf32",
        limine_crate: "not-linked-multiboot1",
        elf_sha256: digest,
    })
}
fn refusal(reason: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(reason, detail)
}
