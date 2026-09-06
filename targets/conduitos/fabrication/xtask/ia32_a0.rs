use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    profile::{Paths, IA32_LINK_PROFILE, IA32_OBJECT_TARGET, IA32_SHARED_BACKBONE_TARGET},
    report::{git_head, sha256_file, ArtifactRole, BuildRecord},
    ConduitosArch, ConduitosError,
};

const ENTRY: &str = "conduitos_ia32_a0_start";
const ARTIFACT_TARGET: &str = "i686-freestanding-elf32";
const ET_EXEC: u16 = 2;
const EM_386: u16 = 3;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const MULTIBOOT1_MAGIC: u32 = 0x1bad_b002;
const MULTIBOOT1_FLAGS: u32 = 4;
const MULTIBOOT1_CHECKSUM: u32 = 0xe452_4ffa;

#[derive(Serialize)]
struct Inspection {
    schema: &'static str,
    proof_class: &'static str,
    architecture: &'static str,
    shared_backbone_target: &'static str,
    shared_backbone_artifact_claimed: bool,
    rust_object_target: &'static str,
    executable_artifact_target: &'static str,
    link_profile: &'static str,
    elf_class: &'static str,
    byte_order: &'static str,
    machine: &'static str,
    entry_symbol: &'static str,
    entry_address: u32,
    load_segment_flags: Vec<u32>,
    required_sections: Vec<&'static str>,
    hosted_interpreter: bool,
    dynamic_linkage: bool,
    runtime_bases_available: bool,
    boot_claimed: bool,
    elf_sha256: String,
}

pub fn execute(opts: &GlobalOpts) -> Result<BuildRecord, ConduitosError> {
    let paths = Paths::new(ConduitosArch::Ia32)?;
    if opts.dry_run {
        println!("rustc {IA32_OBJECT_TARGET} object; rust-lld -m elf_i386");
        return record(&paths, "dry-run".into());
    }
    fs::create_dir_all(&paths.target)
        .map_err(|error| refusal("build-output-unavailable", error.to_string()))?;
    check_shared_backbone(&paths, opts)?;
    let object = paths.target.join("ia32-entry.o");
    compile_object(&paths, &object)?;
    link(&paths, &object)?;
    let bytes = fs::read(&paths.kernel)
        .map_err(|error| refusal("artifact-unavailable", error.to_string()))?;
    let facts = inspect_elf(&bytes)?;
    inspect_readelf(&paths)?;
    let digest = sha256_file(&paths.kernel)?;
    let inspection = Inspection {
        schema: "conduit.conduitos.ia32-a0/v1",
        proof_class: "compile-link-artifact-only",
        architecture: "ia32",
        shared_backbone_target: IA32_SHARED_BACKBONE_TARGET,
        shared_backbone_artifact_claimed: false,
        rust_object_target: IA32_OBJECT_TARGET,
        executable_artifact_target: ARTIFACT_TARGET,
        link_profile: IA32_LINK_PROFILE,
        elf_class: "ELF32",
        byte_order: "little-endian",
        machine: "Intel 80386",
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
    let encoded = serde_json::to_vec_pretty(&inspection)
        .map_err(|error| refusal("build-record-failed", error.to_string()))?;
    fs::write(paths.target.join("a0-inspection.json"), encoded)
        .map_err(|error| refusal("build-record-failed", error.to_string()))?;
    let record = record(&paths, digest)?;
    let encoded = serde_json::to_vec_pretty(&record)
        .map_err(|error| refusal("build-record-failed", error.to_string()))?;
    fs::write(paths.target.join("build.json"), encoded)
        .map_err(|error| refusal("build-record-failed", error.to_string()))?;
    if !opts.quiet && !opts.json {
        println!("ConduitOS IA-32 A0 ELF: {}", paths.kernel.display());
    }
    Ok(record)
}

pub(super) fn check_shared_backbone(
    paths: &Paths,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    let mut command = Command::new("cargo");
    command
        .args([
            "check",
            "-p",
            "conduitos",
            "--lib",
            "--target",
            IA32_SHARED_BACKBONE_TARGET,
        ])
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
            "i686-unknown-uefi shared backbone did not compile",
        ))
    }
}

fn compile_object(paths: &Paths, object: &Path) -> Result<(), ConduitosError> {
    let source = paths
        .root
        .join("targets/conduitos/proof/appliances/ia32/a0.rs");
    let base_commit = git_head(&paths.root)?;
    let status = Command::new("rustc")
        .args([
            "--crate-name",
            "conduitos_ia32_a0",
            "--crate-type",
            "bin",
            "--edition",
            "2024",
            "--target",
            IA32_OBJECT_TARGET,
            "--emit",
            &format!("obj={}", object.display()),
            "-C",
            "panic=abort",
            "-C",
            "relocation-model=static",
            "-C",
            "code-model=kernel",
            "-O",
        ])
        .arg(source)
        .current_dir(&paths.root)
        .env("CONDUITOS_BUILD_ID", &base_commit)
        .env(
            "CONDUITOS_IMAGE_ID",
            format!("conduitos-image/{base_commit}/ia32/v1"),
        )
        .status()
        .map_err(|error| refusal("ia32-object-toolchain-unavailable", error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(refusal("ia32-object-compile-failed", status.to_string()))
    }
}

fn link(paths: &Paths, object: &Path) -> Result<(), ConduitosError> {
    let linker = rust_lld(&paths.root)?;
    let script = paths
        .root
        .join("targets/conduitos/proof/appliances/ia32/linker/a0.ld");
    let status = Command::new(linker)
        .args(["-flavor", "gnu", "-m", "elf_i386", "-T"])
        .arg(script)
        .args(["-o"])
        .arg(&paths.kernel)
        .arg(object)
        .current_dir(&paths.root)
        .status()
        .map_err(|error| refusal("ia32-linker-unavailable", error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(refusal("ia32-link-failed", status.to_string()))
    }
}

pub(super) fn rust_lld(root: &Path) -> Result<PathBuf, ConduitosError> {
    let sysroot = output(
        "rustc",
        &["--print", "sysroot"],
        root,
        "ia32-object-toolchain-unavailable",
    )?;
    let version = output("rustc", &["-vV"], root, "ia32-object-toolchain-unavailable")?;
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| {
            refusal(
                "ia32-object-toolchain-unavailable",
                "rustc host identity is absent",
            )
        })?;
    let path = PathBuf::from(sysroot.trim())
        .join("lib/rustlib")
        .join(host)
        .join("bin/rust-lld");
    if path.is_file() {
        Ok(path)
    } else {
        Err(refusal(
            "ia32-linker-unavailable",
            path.display().to_string(),
        ))
    }
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
        || symbols
            .lines()
            .any(|line| line.contains("conduitos_start") || line.contains("aarch64"))
    {
        return Err(invalid(
            "exact IA-32 entry is absent or another architecture leaked into symbols",
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
            return Err(invalid(format!("required section {section} is absent")));
        }
    }
    Ok(())
}

pub(super) struct ElfFacts {
    entry: u32,
    load_flags: Vec<u32>,
}

pub(super) fn inspect_elf(bytes: &[u8]) -> Result<ElfFacts, ConduitosError> {
    if bytes.len() < 52
        || &bytes[..4] != b"\x7fELF"
        || bytes[4] != 1
        || bytes[5] != 1
        || bytes[6] != 1
        || !matches!(bytes[7], 0 | 3)
    {
        return Err(invalid("expected one little-endian ELF32 artifact"));
    }
    if u16_at(bytes, 16)? != ET_EXEC || u16_at(bytes, 18)? != EM_386 {
        return Err(invalid(
            "artifact is not an IA-32 executable; PE and x86_64 aliases are rejected",
        ));
    }
    let entry = u32_at(bytes, 24)?;
    if entry == 0 || u32_at(bytes, 36)? != 0 || u16_at(bytes, 40)? != 52 || u16_at(bytes, 42)? < 32
    {
        return Err(invalid("entry or IA-32 ELF ABI facts are malformed"));
    }
    let header_limit = bytes.len().min(8192);
    let multiboot_offset = bytes[..header_limit]
        .windows(4)
        .position(|value| value == MULTIBOOT1_MAGIC.to_le_bytes())
        .filter(|offset| offset % 4 == 0)
        .ok_or_else(|| invalid("exact Multiboot1 header is absent from the first 8192 bytes"))?;
    if u32_at(bytes, multiboot_offset + 4)? != MULTIBOOT1_FLAGS
        || u32_at(bytes, multiboot_offset + 8)? != MULTIBOOT1_CHECKSUM
    {
        return Err(invalid("Multiboot1 profile flags or checksum are stale"));
    }
    let phoff =
        usize::try_from(u32_at(bytes, 28)?).map_err(|_| invalid("program table overflow"))?;
    let phsize = usize::from(u16_at(bytes, 42)?);
    let phnum = usize::from(u16_at(bytes, 44)?);
    let mut load_flags = Vec::new();
    for index in 0..phnum {
        let offset = phoff
            .checked_add(
                index
                    .checked_mul(phsize)
                    .ok_or_else(|| invalid("program table overflow"))?,
            )
            .filter(|offset| offset.saturating_add(32) <= bytes.len())
            .ok_or_else(|| invalid("program table is out of bounds"))?;
        let kind = u32_at(bytes, offset)?;
        if matches!(kind, PT_DYNAMIC | PT_INTERP) {
            return Err(invalid("hosted interpreter or dynamic linkage is present"));
        }
        if kind == PT_LOAD {
            let flags = u32_at(bytes, offset + 24)?;
            if flags & 3 == 3 {
                return Err(invalid("writable executable LOAD segment is forbidden"));
            }
            let file_end = usize::try_from(u32_at(bytes, offset + 4)?)
                .ok()
                .and_then(|start| {
                    start.checked_add(usize::try_from(u32_at(bytes, offset + 16).ok()?).ok()?)
                });
            if !file_end.is_some_and(|end| end <= bytes.len()) {
                return Err(invalid("LOAD segment exceeds artifact"));
            }
            load_flags.push(flags);
        }
    }
    if load_flags != [5, 4, 6] {
        return Err(invalid(format!(
            "expected exact R+X/R/R+W LOAD flags, found {load_flags:?}"
        )));
    }
    Ok(ElfFacts { entry, load_flags })
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
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
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
        architecture: "ia32",
        rust_target: ARTIFACT_TARGET,
        limine_crate: "not-linked-a0",
        elf_sha256: digest,
    })
}
fn invalid(detail: impl Into<String>) -> ConduitosError {
    refusal("invalid-ia32-a0-artifact", detail)
}
fn refusal(reason: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(reason, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specimen() -> Vec<u8> {
        let mut bytes = vec![0_u8; 256];
        bytes[..8].copy_from_slice(b"\x7fELF\x01\x01\x01\x00");
        bytes[16..20].copy_from_slice(&[2, 0, 3, 0]);
        bytes[24..28].copy_from_slice(&0xc010_0000_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&52_u32.to_le_bytes());
        bytes[40..44].copy_from_slice(&[52, 0, 32, 0]);
        bytes[44..46].copy_from_slice(&3_u16.to_le_bytes());
        bytes[148..152].copy_from_slice(&MULTIBOOT1_MAGIC.to_le_bytes());
        bytes[152..156].copy_from_slice(&MULTIBOOT1_FLAGS.to_le_bytes());
        bytes[156..160].copy_from_slice(&MULTIBOOT1_CHECKSUM.to_le_bytes());
        for (index, flags) in [5_u32, 4, 6].into_iter().enumerate() {
            let offset = 52 + index * 32;
            bytes[offset..offset + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
            bytes[offset + 4..offset + 8].copy_from_slice(&(160 + index as u32 * 16).to_le_bytes());
            bytes[offset + 16..offset + 20].copy_from_slice(&8_u32.to_le_bytes());
            bytes[offset + 24..offset + 28].copy_from_slice(&flags.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn wrong_class_machine_and_hosted_segments_refuse() {
        let mut bytes = specimen();
        assert!(inspect_elf(&bytes).is_ok());
        bytes[4] = 2;
        assert!(inspect_elf(&bytes).is_err());
        bytes = specimen();
        bytes[18] = 62;
        assert!(inspect_elf(&bytes).is_err());
        bytes = specimen();
        bytes[52..56].copy_from_slice(&PT_INTERP.to_le_bytes());
        assert!(inspect_elf(&bytes).is_err());
        bytes = specimen();
        bytes[52 + 24..52 + 28].copy_from_slice(&7_u32.to_le_bytes());
        assert!(inspect_elf(&bytes).is_err());
        bytes = specimen();
        bytes[24..28].fill(0);
        assert!(inspect_elf(&bytes).is_err());
        bytes = specimen();
        bytes[156..160].fill(0);
        assert!(inspect_elf(&bytes).is_err());
    }

    #[test]
    fn shared_and_artifact_profiles_are_explicitly_distinct() {
        assert_eq!(IA32_SHARED_BACKBONE_TARGET, "i686-unknown-uefi");
        assert_eq!(IA32_OBJECT_TARGET, "i686-unknown-linux-gnu");
        assert_ne!(IA32_SHARED_BACKBONE_TARGET, ARTIFACT_TARGET);
        assert!(IA32_LINK_PROFILE.contains("elf_i386"));
    }
}
