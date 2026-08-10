use std::{fs, path::Path, process::Command};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    profile::Paths,
    report::{git_head, sha256_file, BuildRecord},
    ConduitosArch, ConduitosError,
};

pub const TARGET: &str = "aarch64-unknown-none";
const BINARY: &str = "conduitos-aarch64-a0";
const MACHINE_AARCH64: u16 = 183;
const ET_EXEC: u16 = 2;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;

#[derive(Debug, Serialize)]
struct A0Inspection {
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
    required_sections: Vec<String>,
    hosted_interpreter: bool,
    dynamic_linkage: bool,
    runtime_bases_available: bool,
    boot_claimed: bool,
    elf_sha256: String,
}

pub fn execute(opts: &GlobalOpts) -> Result<BuildRecord, ConduitosError> {
    assert_profile("aarch64", TARGET)?;
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    if opts.dry_run {
        println!(
            "cargo build -p conduitos --bin {BINARY} --features aarch64-a0 --target {TARGET} --release"
        );
        return record(&paths, "dry-run".to_owned());
    }
    fs::create_dir_all(&paths.target)
        .map_err(|error| ConduitosError::refusal("build-output-unavailable", error.to_string()))?;
    check_shared_backbone(&paths, opts)?;
    let mut command = Command::new("cargo");
    let base_commit = git_head(&paths.root)?;
    command
        .args([
            "build",
            "-p",
            "conduitos",
            "--bin",
            BINARY,
            "--features",
            "aarch64-a0",
            "--target",
            TARGET,
            "--release",
        ])
        .current_dir(&paths.root)
        .env("RUSTFLAGS", "-C relocation-model=static -C panic=abort")
        .env(
            "CONDUITOS_BUILD_ID",
            format!("conduitos-build/{base_commit}/aarch64/v1"),
        )
        .env(
            "CONDUITOS_IMAGE_ID",
            format!("conduitos-image/{base_commit}/aarch64/v1"),
        );
    if opts.locked {
        command.arg("--locked");
    }
    let status = command.status().map_err(|error| {
        ConduitosError::refusal(
            "toolchain-unavailable",
            format!("cannot launch cargo: {error}"),
        )
    })?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "compile-link-failed",
            status.to_string(),
        ));
    }
    let built = paths.root.join(format!("target/{TARGET}/release/{BINARY}"));
    fs::copy(&built, &paths.kernel)
        .map_err(|error| ConduitosError::refusal("build-output-unavailable", error.to_string()))?;
    let bytes = fs::read(&paths.kernel)
        .map_err(|error| ConduitosError::refusal("artifact-unavailable", error.to_string()))?;
    let facts = inspect_elf(&bytes)?;
    let symbols = super::profile::command(
        "readelf",
        &["-sW", path_text(&paths.kernel)?],
        &paths.root,
        "readelf-unavailable",
    )?;
    let symbols = String::from_utf8_lossy(&symbols.stdout);
    assert_symbol_contract(&symbols)?;
    let digest = sha256_file(&paths.kernel)?;
    let inspection = A0Inspection {
        schema: "conduit.conduitos.aarch64-a0/v1",
        proof_class: "compile-link-artifact-only",
        architecture: "aarch64",
        rust_target: TARGET,
        elf_class: "ELF64",
        byte_order: "little-endian",
        machine: "AArch64",
        abi_flags: facts.abi_flags,
        entry_symbol: "conduitos_aarch64_a0_start",
        entry_address: facts.entry,
        required_sections: facts.sections,
        hosted_interpreter: false,
        dynamic_linkage: false,
        runtime_bases_available: false,
        boot_claimed: false,
        elf_sha256: digest.clone(),
    };
    let inspection_bytes = serde_json::to_vec_pretty(&inspection)
        .map_err(|error| ConduitosError::refusal("build-record-failed", error.to_string()))?;
    fs::write(paths.target.join("a0-inspection.json"), inspection_bytes)
        .map_err(|error| ConduitosError::refusal("build-record-failed", error.to_string()))?;
    let record = record(&paths, digest)?;
    let record_bytes = serde_json::to_vec_pretty(&record)
        .map_err(|error| ConduitosError::refusal("build-record-failed", error.to_string()))?;
    fs::write(paths.target.join("build.json"), record_bytes)
        .map_err(|error| ConduitosError::refusal("build-record-failed", error.to_string()))?;
    if !opts.quiet && !opts.json {
        println!("ConduitOS AArch64 A0 ELF: {}", paths.kernel.display());
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
    let status = command.status().map_err(|error| {
        ConduitosError::refusal(
            "matrix-toolchain-unavailable",
            format!("cannot check shared AArch64 backbone: {error}"),
        )
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(ConduitosError::refusal(
            "matrix-common-backbone-failed",
            "shared ConduitOS backbone did not compile for aarch64-unknown-none",
        ))
    }
}

fn record(paths: &Paths, digest: String) -> Result<BuildRecord, ConduitosError> {
    Ok(BuildRecord {
        schema: "conduit.conduitos.build/v1",
        base_commit: git_head(&paths.root)?,
        architecture: "aarch64",
        rust_target: TARGET,
        limine_crate: "0.5.0",
        elf_sha256: digest,
    })
}

fn path_text(path: &Path) -> Result<&str, ConduitosError> {
    path.to_str().ok_or_else(|| {
        ConduitosError::refusal("build-output-unavailable", "non-UTF-8 artifact path")
    })
}

#[derive(Debug)]
struct ElfFacts {
    entry: u64,
    abi_flags: u32,
    sections: Vec<String>,
}

fn inspect_elf(bytes: &[u8]) -> Result<ElfFacts, ConduitosError> {
    if bytes.len() < 64
        || &bytes[..4] != b"\x7fELF"
        || bytes[4] != 2
        || bytes[5] != 1
        || bytes[6] != 1
        || bytes[7] != 0
    {
        return Err(invalid("expected a little-endian ELF64 artifact"));
    }
    if u16_at(bytes, 16)? != ET_EXEC {
        return Err(invalid("artifact is not an executable ELF"));
    }
    if u16_at(bytes, 18)? != MACHINE_AARCH64 {
        return Err(invalid(
            "artifact machine is not AArch64; x86 aliases are rejected",
        ));
    }
    let entry = u64_at(bytes, 24)?;
    if entry == 0 {
        return Err(invalid("artifact has no entry address"));
    }
    let program_offset = usize_at(bytes, 32)?;
    let section_offset = usize_at(bytes, 40)?;
    let abi_flags = u32_at(bytes, 48)?;
    if abi_flags != 0 {
        return Err(invalid("unexpected AArch64 ABI flags"));
    }
    let program_size = usize::from(u16_at(bytes, 54)?);
    let program_count = usize::from(u16_at(bytes, 56)?);
    let section_size = usize::from(u16_at(bytes, 58)?);
    let section_count = usize::from(u16_at(bytes, 60)?);
    let names_index = usize::from(u16_at(bytes, 62)?);
    if program_size < 56 || section_size < 64 || names_index >= section_count {
        return Err(invalid(
            "artifact header table identity is stale or malformed",
        ));
    }
    for index in 0..program_count {
        let offset = table_offset(program_offset, program_size, index, bytes.len())?;
        if matches!(u32_at(bytes, offset)?, PT_DYNAMIC | PT_INTERP) {
            return Err(invalid("hosted interpreter or dynamic linkage is present"));
        }
    }
    let names_header = table_offset(section_offset, section_size, names_index, bytes.len())?;
    let names_offset = usize_at(bytes, names_header + 24)?;
    let names_size = usize_at(bytes, names_header + 32)?;
    let names_end = names_offset
        .checked_add(names_size)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| invalid("section-name table is out of bounds"))?;
    let names = &bytes[names_offset..names_end];
    let mut sections = Vec::new();
    for index in 0..section_count {
        let offset = table_offset(section_offset, section_size, index, bytes.len())?;
        let name = c_string(
            names,
            usize::try_from(u32_at(bytes, offset)?).map_err(|_| invalid("invalid section name"))?,
        )?;
        if matches!(name, ".dynamic" | ".dynsym" | ".dynstr" | ".interp") {
            return Err(invalid("hosted or dynamic-link section is present"));
        }
        if matches!(name, ".text" | ".symtab" | ".strtab" | ".shstrtab") {
            sections.push(name.to_owned());
        }
    }
    for required in [".text", ".symtab", ".strtab", ".shstrtab"] {
        if !sections.iter().any(|name| name == required) {
            return Err(invalid(format!(
                "required linked section {required} is missing"
            )));
        }
    }
    Ok(ElfFacts {
        entry,
        abi_flags,
        sections,
    })
}

fn assert_profile(architecture: &str, target: &str) -> Result<(), ConduitosError> {
    if architecture == "aarch64" && target == TARGET {
        Ok(())
    } else {
        Err(ConduitosError::refusal(
            "stale-aarch64-a0-profile",
            format!("expected aarch64/{TARGET}, found {architecture}/{target}"),
        ))
    }
}

fn assert_symbol_contract(symbols: &str) -> Result<(), ConduitosError> {
    if !symbols.lines().any(|line| {
        line.contains("GLOBAL")
            && line.contains("conduitos_aarch64_a0_start")
            && !line.contains(" UND ")
    }) {
        return Err(invalid("linked AArch64 entry symbol is missing"));
    }
    let forbidden = ["cpuid", "pic_", "pit_", "gdt", "idt", "com1", "serial"];
    if let Some(symbol) = forbidden
        .iter()
        .find(|symbol| symbols.to_ascii_lowercase().contains(*symbol))
    {
        return Err(invalid(format!(
            "x86-specific symbol leaked into A0 artifact: {symbol}"
        )));
    }
    Ok(())
}

fn invalid(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("invalid-aarch64-a0-artifact", detail)
}

fn table_offset(
    base: usize,
    size: usize,
    index: usize,
    length: usize,
) -> Result<usize, ConduitosError> {
    base.checked_add(
        size.checked_mul(index)
            .ok_or_else(|| invalid("ELF table overflow"))?,
    )
    .filter(|offset| offset.checked_add(size).is_some_and(|end| end <= length))
    .ok_or_else(|| invalid("ELF table is out of bounds"))
}

fn c_string(bytes: &[u8], offset: usize) -> Result<&str, ConduitosError> {
    let tail = bytes
        .get(offset..)
        .ok_or_else(|| invalid("section name is out of bounds"))?;
    let end = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| invalid("unterminated section name"))?;
    std::str::from_utf8(&tail[..end]).map_err(|_| invalid("section name is not UTF-8"))
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, ConduitosError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| invalid("truncated ELF"))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, ConduitosError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| invalid("truncated ELF"))?;
    Ok(u32::from_le_bytes(
        value.try_into().expect("four-byte slice"),
    ))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, ConduitosError> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| invalid("truncated ELF"))?;
    Ok(u64::from_le_bytes(
        value.try_into().expect("eight-byte slice"),
    ))
}

fn usize_at(bytes: &[u8], offset: usize) -> Result<usize, ConduitosError> {
    usize::try_from(u64_at(bytes, offset)?).map_err(|_| invalid("ELF offset exceeds host size"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(machine: u16, entry: u64, include_text: bool) -> Vec<u8> {
        let names = if include_text {
            b"\0.text\0.symtab\0.strtab\0.shstrtab\0".as_slice()
        } else {
            b"\0.symtab\0.strtab\0.shstrtab\0".as_slice()
        };
        let count = if include_text { 5 } else { 4 };
        let section_offset = 64usize;
        let names_offset = section_offset + count * 64;
        let mut elf = vec![0u8; names_offset + names.len()];
        elf[..6].copy_from_slice(b"\x7fELF\x02\x01");
        elf[6] = 1;
        elf[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
        elf[18..20].copy_from_slice(&machine.to_le_bytes());
        elf[24..32].copy_from_slice(&entry.to_le_bytes());
        elf[40..48].copy_from_slice(&(section_offset as u64).to_le_bytes());
        elf[54..56].copy_from_slice(&56u16.to_le_bytes());
        elf[58..60].copy_from_slice(&64u16.to_le_bytes());
        elf[60..62].copy_from_slice(&(count as u16).to_le_bytes());
        elf[62..64].copy_from_slice(&((count - 1) as u16).to_le_bytes());
        let mut offsets = if include_text {
            vec![0, 1, 7, 15, 23]
        } else {
            vec![0, 1, 9, 17]
        };
        for (index, name) in offsets.drain(..).enumerate() {
            let header = section_offset + index * 64;
            elf[header..header + 4].copy_from_slice(&(name as u32).to_le_bytes());
        }
        let names_header = section_offset + (count - 1) * 64;
        elf[names_header + 24..names_header + 32]
            .copy_from_slice(&(names_offset as u64).to_le_bytes());
        elf[names_header + 32..names_header + 40]
            .copy_from_slice(&(names.len() as u64).to_le_bytes());
        elf[names_offset..].copy_from_slice(names);
        elf
    }

    #[test]
    fn accepts_exact_aarch64_compile_link_identity() {
        let facts = inspect_elf(&fixture(MACHINE_AARCH64, 0x4000_0000, true)).unwrap();
        assert_eq!(facts.entry, 0x4000_0000);
    }

    #[test]
    fn rejects_x86_alias_wrong_machine() {
        assert!(inspect_elf(&fixture(62, 0x4000_0000, true)).is_err());
    }

    #[test]
    fn rejects_missing_entry_or_linked_text() {
        assert!(inspect_elf(&fixture(MACHINE_AARCH64, 0, true)).is_err());
        assert!(inspect_elf(&fixture(MACHINE_AARCH64, 0x4000_0000, false)).is_err());
    }

    #[test]
    fn target_profile_identity_is_exact() {
        assert_profile("aarch64", "aarch64-unknown-none").unwrap();
        assert!(assert_profile("aarch64", "x86_64-unknown-none").is_err());
        assert!(assert_profile("x86_64", "aarch64-unknown-none").is_err());
    }

    #[test]
    fn entry_symbol_and_x86_leak_contract_is_negative() {
        assert_symbol_contract(
            "1: 0000000040000000 4 FUNC GLOBAL DEFAULT 1 conduitos_aarch64_a0_start",
        )
        .unwrap();
        assert!(assert_symbol_contract("1: 0 4 FUNC GLOBAL DEFAULT 1 cpuid").is_err());
    }
}
