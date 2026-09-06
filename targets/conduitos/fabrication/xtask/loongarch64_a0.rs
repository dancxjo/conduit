use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    profile::Paths,
    report::{git_head, sha256_file, ArtifactRole, BuildRecord},
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use serde::Serialize;

pub const TARGET: &str = "loongarch64-unknown-none";
const ENTRY: &str = "conduitos_loongarch64_a0_start";
const EM_LOONGARCH: u16 = 258;
const EF_LOONGARCH_DOUBLE_FLOAT_OBJABI_V1: u32 = 0x43;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;

#[derive(Serialize)]
struct Inspection {
    schema: &'static str,
    proof_class: &'static str,
    architecture: &'static str,
    rust_target: &'static str,
    elf_class: &'static str,
    byte_order: &'static str,
    machine: &'static str,
    abi_flags: u32,
    entry_symbol: &'static str,
    entry_address: u64,
    load_segment_flags: Vec<u32>,
    required_sections: Vec<&'static str>,
    hosted_interpreter: bool,
    dynamic_linkage: bool,
    runtime_bases_available: bool,
    boot_claimed: bool,
    elf_sha256: String,
}

pub fn execute(opts: &GlobalOpts) -> Result<BuildRecord, ConduitosError> {
    let paths = Paths::new(ConduitosArch::Loongarch64)?;
    if opts.dry_run {
        println!("rustc --target {TARGET} object; rust-lld -m elf64loongarch");
        return record(&paths, "dry-run".into());
    }
    fs::create_dir_all(&paths.target)
        .map_err(|error| refusal("build-output-unavailable", error.to_string()))?;
    check_shared_backbone(&paths, opts)?;
    let object = paths.target.join("loongarch64-entry.o");
    compile_object(&paths, &object)?;
    link(&paths, &object)?;
    let bytes = fs::read(&paths.kernel)
        .map_err(|error| refusal("artifact-unavailable", error.to_string()))?;
    let facts = inspect_elf(&bytes)?;
    inspect_readelf(&paths)?;
    let digest = sha256_file(&paths.kernel)?;
    let inspection = Inspection {
        schema: "conduit.conduitos.loongarch64-a0/v1",
        proof_class: "compile-link-artifact-only",
        architecture: "loongarch64",
        rust_target: TARGET,
        elf_class: "ELF64",
        byte_order: "little-endian",
        machine: "LoongArch",
        abi_flags: facts.abi_flags,
        entry_symbol: ENTRY,
        entry_address: facts.entry,
        load_segment_flags: facts.load_flags,
        required_sections: vec![
            ".text",
            ".rodata",
            ".bss",
            ".symtab",
            ".strtab",
            ".shstrtab",
        ],
        hosted_interpreter: false,
        dynamic_linkage: false,
        runtime_bases_available: false,
        boot_claimed: false,
        elf_sha256: digest.clone(),
    };
    fs::write(
        paths.target.join("a0-inspection.json"),
        serde_json::to_vec_pretty(&inspection)
            .map_err(|error| refusal("build-record-failed", error.to_string()))?,
    )
    .map_err(|error| refusal("build-record-failed", error.to_string()))?;
    let record = record(&paths, digest)?;
    fs::write(
        paths.target.join("build.json"),
        serde_json::to_vec_pretty(&record)
            .map_err(|error| refusal("build-record-failed", error.to_string()))?,
    )
    .map_err(|error| refusal("build-record-failed", error.to_string()))?;
    if !opts.quiet && !opts.json {
        println!("ConduitOS LoongArch64 A0 ELF: {}", paths.kernel.display());
    }
    Ok(record)
}

fn check_shared_backbone(paths: &Paths, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let mut command = Command::new("cargo");
    command
        .args(["check", "-p", "conduitos", "--lib", "--target", TARGET])
        .current_dir(&paths.root);
    if opts.locked {
        command.arg("--locked");
    }
    let status = command
        .status()
        .map_err(|error| refusal("matrix-toolchain-unavailable", error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(refusal(
            "matrix-common-backbone-failed",
            "LoongArch64 shared backbone did not compile",
        ))
    }
}

fn compile_object(paths: &Paths, object: &Path) -> Result<(), ConduitosError> {
    let source = paths
        .root
        .join("targets/conduitos/proof/appliances/loongarch64/a0.rs");
    let commit = git_head(&paths.root)?;
    let status = Command::new("rustc")
        .args([
            "--crate-name",
            "conduitos_loongarch64_a0",
            "--crate-type",
            "bin",
            "--edition",
            "2024",
            "--target",
            TARGET,
            "--emit",
        ])
        .arg(format!("obj={}", object.display()))
        .args(["-C", "panic=abort", "-C", "relocation-model=static", "-O"])
        .arg(source)
        .env("CONDUITOS_BUILD_ID", &commit)
        .env(
            "CONDUITOS_IMAGE_ID",
            format!("conduitos-image/{commit}/loongarch64/v1"),
        )
        .current_dir(&paths.root)
        .status()
        .map_err(|error| {
            refusal(
                "loongarch64-object-toolchain-unavailable",
                error.to_string(),
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(refusal(
            "loongarch64-object-compile-failed",
            status.to_string(),
        ))
    }
}

fn link(paths: &Paths, object: &Path) -> Result<(), ConduitosError> {
    let linker = rust_lld(&paths.root)?;
    let script = paths
        .root
        .join("targets/conduitos/proof/appliances/loongarch64/linker/a0.ld");
    let status = Command::new(linker)
        .args(["-flavor", "gnu", "-m", "elf64loongarch", "-T"])
        .arg(script)
        .arg("-o")
        .arg(&paths.kernel)
        .arg(object)
        .current_dir(&paths.root)
        .status()
        .map_err(|error| refusal("loongarch64-linker-unavailable", error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(refusal("loongarch64-link-failed", status.to_string()))
    }
}

pub(super) fn rust_lld(root: &Path) -> Result<PathBuf, ConduitosError> {
    let sysroot = output(
        "rustc",
        &["--print", "sysroot"],
        root,
        "loongarch64-object-toolchain-unavailable",
    )?;
    let version = output(
        "rustc",
        &["-vV"],
        root,
        "loongarch64-object-toolchain-unavailable",
    )?;
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| {
            refusal(
                "loongarch64-object-toolchain-unavailable",
                "rustc host identity absent",
            )
        })?;
    let path = PathBuf::from(sysroot.trim())
        .join("lib/rustlib")
        .join(host)
        .join("bin/rust-lld");
    path.is_file()
        .then_some(path.clone())
        .ok_or_else(|| refusal("loongarch64-linker-unavailable", path.display().to_string()))
}

fn inspect_readelf(paths: &Paths) -> Result<(), ConduitosError> {
    let kernel = paths
        .kernel
        .to_str()
        .ok_or_else(|| refusal("artifact-unavailable", "non-UTF-8 artifact path"))?;
    let symbols = output(
        "readelf",
        &["-sW", kernel],
        &paths.root,
        "readelf-unavailable",
    )?;
    if !symbols
        .lines()
        .any(|line| line.contains("GLOBAL") && line.ends_with(ENTRY))
        || symbols.contains("conduitos_start")
        || symbols.contains("aarch64")
        || symbols.contains("ia32")
        || symbols.contains("riscv")
    {
        return Err(invalid(
            "exact LoongArch64 entry absent or another architecture leaked into symbols",
        ));
    }
    let sections = output(
        "readelf",
        &["-SW", kernel],
        &paths.root,
        "readelf-unavailable",
    )?;
    for section in [
        ".text",
        ".rodata",
        ".bss",
        ".symtab",
        ".strtab",
        ".shstrtab",
    ] {
        if !sections.contains(section) {
            return Err(invalid(format!("required section {section} absent")));
        }
    }
    Ok(())
}

struct ElfFacts {
    entry: u64,
    abi_flags: u32,
    load_flags: Vec<u32>,
}

fn inspect_elf(bytes: &[u8]) -> Result<ElfFacts, ConduitosError> {
    if bytes.len() < 64 || &bytes[..4] != b"\x7fELF" || bytes[4..8] != [2, 1, 1, 0] {
        return Err(invalid("expected little-endian ELF64"));
    }
    if u16_at(bytes, 16)? != 2 || u16_at(bytes, 18)? != EM_LOONGARCH {
        return Err(invalid("artifact is not a LoongArch64 executable"));
    }
    let entry = u64_at(bytes, 24)?;
    let abi_flags = u32_at(bytes, 48)?;
    if entry == 0
        || abi_flags != EF_LOONGARCH_DOUBLE_FLOAT_OBJABI_V1
        || u16_at(bytes, 52)? != 64
        || u16_at(bytes, 54)? < 56
    {
        return Err(invalid("entry or LoongArch ELF ABI facts malformed"));
    }
    let phoff =
        usize::try_from(u64_at(bytes, 32)?).map_err(|_| invalid("program table overflow"))?;
    let phsize = usize::from(u16_at(bytes, 54)?);
    let phnum = usize::from(u16_at(bytes, 56)?);
    let mut load_flags = Vec::new();
    for index in 0..phnum {
        let offset = phoff
            .checked_add(
                index
                    .checked_mul(phsize)
                    .ok_or_else(|| invalid("program table overflow"))?,
            )
            .filter(|offset| offset.saturating_add(56) <= bytes.len())
            .ok_or_else(|| invalid("program table out of bounds"))?;
        let kind = u32_at(bytes, offset)?;
        if matches!(kind, PT_DYNAMIC | PT_INTERP) {
            return Err(invalid("hosted interpreter or dynamic linkage present"));
        }
        if kind == PT_LOAD {
            let flags = u32_at(bytes, offset + 4)?;
            if flags & 3 == 3 {
                return Err(invalid("writable executable LOAD segment forbidden"));
            }
            let end = usize::try_from(u64_at(bytes, offset + 8)?)
                .ok()
                .and_then(|start| {
                    start.checked_add(usize::try_from(u64_at(bytes, offset + 32).ok()?).ok()?)
                });
            if !end.is_some_and(|end| end <= bytes.len()) {
                return Err(invalid("LOAD segment exceeds artifact"));
            }
            load_flags.push(flags);
        }
    }
    if load_flags != [5, 4, 6] {
        return Err(invalid(format!(
            "expected R+X/R/R+W LOAD flags, found {load_flags:?}"
        )));
    }
    Ok(ElfFacts {
        entry,
        abi_flags,
        load_flags,
    })
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, ConduitosError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| invalid("truncated ELF field"))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}
fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, ConduitosError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| invalid("truncated ELF field"))?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}
fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, ConduitosError> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| invalid("truncated ELF field"))?;
    Ok(u64::from_le_bytes(value.try_into().unwrap()))
}
fn output(
    program: &str,
    args: &[&str],
    root: &Path,
    reason: &'static str,
) -> Result<String, ConduitosError> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| refusal(reason, error.to_string()))?;
    if !output.status.success() {
        return Err(refusal(reason, String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
fn record(paths: &Paths, digest: String) -> Result<BuildRecord, ConduitosError> {
    Ok(BuildRecord {
        schema: "conduit.conduitos.build/v2",
        artifact_role: ArtifactRole::ArchitectureProofAppliance,
        base_commit: git_head(&paths.root)?,
        architecture: "loongarch64",
        rust_target: TARGET,
        limine_crate: "not-linked-a0",
        elf_sha256: digest,
    })
}
fn invalid(detail: impl Into<String>) -> ConduitosError {
    refusal("invalid-loongarch64-a0-artifact", detail)
}
fn refusal(reason: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(reason, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_wrong_machine_class_hosted_and_writable_executable() {
        let mut bytes = vec![0_u8; 256];
        bytes[..8].copy_from_slice(b"\x7fELF\x02\x01\x01\x00");
        bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&EM_LOONGARCH.to_le_bytes());
        bytes[24..32].copy_from_slice(&0xffff_ffff_8000_0000_u64.to_le_bytes());
        bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
        bytes[48..52].copy_from_slice(&EF_LOONGARCH_DOUBLE_FLOAT_OBJABI_V1.to_le_bytes());
        bytes[52..58].copy_from_slice(&[64, 0, 56, 0, 3, 0]);
        for (index, flags) in [5_u32, 4, 6].into_iter().enumerate() {
            let offset = 64 + index * 56;
            bytes[offset..offset + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
            bytes[offset + 4..offset + 8].copy_from_slice(&flags.to_le_bytes());
            bytes[offset + 8..offset + 16]
                .copy_from_slice(&(232_u64 + index as u64 * 8).to_le_bytes());
            bytes[offset + 32..offset + 40].copy_from_slice(&8_u64.to_le_bytes());
        }
        assert!(inspect_elf(&bytes).is_ok());
        bytes[18] = 62;
        assert!(inspect_elf(&bytes).is_err());
        bytes[18..20].copy_from_slice(&EM_LOONGARCH.to_le_bytes());
        bytes[4] = 1;
        assert!(inspect_elf(&bytes).is_err());
        bytes[4] = 2;
        bytes[64..68].copy_from_slice(&PT_INTERP.to_le_bytes());
        assert!(inspect_elf(&bytes).is_err());
    }
}
