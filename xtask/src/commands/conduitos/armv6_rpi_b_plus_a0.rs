use std::{fs, path::Path, process::Command};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    profile::Paths,
    report::{git_head, sha256_file, BuildRecord},
    ConduitosArch, ConduitosError,
};

pub const TARGET: &str = "armv6-none-eabi";
const BINARY: &str = "conduitos-armv6-rpi-b-plus-a0";
const ENTRY: &str = "conduitos_armv6_rpi_b_plus_entry";
const LOAD_ADDRESS: u32 = 0x8000;
const ELF_HEADER_BYTES: usize = 52;
const PROGRAM_HEADER_BYTES: usize = 32;
const ET_EXEC: u16 = 2;
const EM_ARM: u16 = 40;
const PT_LOAD: u32 = 1;

#[derive(Debug, Serialize)]
struct Inspection {
    schema: &'static str,
    proof_class: &'static str,
    architecture: &'static str,
    machine: &'static str,
    board: &'static str,
    rust_target: &'static str,
    boot_path: &'static str,
    entry_symbol: &'static str,
    entry_address: u32,
    load_address: u32,
    elf_class: &'static str,
    byte_order: &'static str,
    eabi_version: u8,
    hard_float_abi: bool,
    hosted_interpreter: bool,
    dynamic_linkage: bool,
    runtime_bases_available: bool,
    boot_claimed: bool,
    elf_sha256: String,
    kernel_image_sha256: String,
    kernel_image_bytes: usize,
}

pub fn execute(opts: &GlobalOpts) -> Result<BuildRecord, ConduitosError> {
    let paths = Paths::new(ConduitosArch::Armv6)?;
    if opts.dry_run {
        println!(
            "cargo build -Zbuild-std=core,alloc -p conduitos --bin {BINARY} --features armv6-rpi-b-plus-a0 --target {TARGET} --release"
        );
        return record(&paths, "dry-run".into());
    }
    fs::create_dir_all(&paths.target)
        .map_err(|error| refusal("build-output-unavailable", error))?;
    let mut command = Command::new("cargo");
    command
        .args([
            "build",
            "-Zbuild-std=core,alloc",
            "-p",
            "conduitos",
            "--bin",
            BINARY,
            "--features",
            "armv6-rpi-b-plus-a0",
            "--target",
            TARGET,
            "--release",
        ])
        .current_dir(&paths.root)
        .env("RUSTC_BOOTSTRAP", "1")
        .env(
            "RUSTFLAGS",
            "-C relocation-model=static -C panic=abort -C opt-level=z -C codegen-units=1",
        );
    if opts.locked {
        command.arg("--locked");
    }
    let status = command
        .status()
        .map_err(|error| refusal("armv6-toolchain-unavailable", error))?;
    if !status.success() {
        return Err(refusal("armv6-compile-link-failed", status));
    }

    let built = paths.root.join(format!("target/{TARGET}/release/{BINARY}"));
    fs::copy(&built, &paths.kernel).map_err(|error| refusal("build-output-unavailable", error))?;
    let elf = fs::read(&paths.kernel).map_err(|error| refusal("artifact-unavailable", error))?;
    let facts = inspect_elf(&elf)?;
    inspect_symbol(&paths.kernel, &paths.root)?;
    let image = flatten_load_image(&elf, &facts.load_segments)?;
    let kernel_image = paths.target.join("kernel.img");
    fs::write(&kernel_image, &image)
        .map_err(|error| refusal("kernel-image-write-failed", error))?;

    let elf_digest = sha256_file(&paths.kernel)?;
    let image_digest = sha256_file(&kernel_image)?;
    let inspection = Inspection {
        schema: "conduit.conduitos.armv6-rpi-b-plus-a0/v1",
        proof_class: "compile-link-artifact-only",
        architecture: "armv6",
        machine: "BCM2835/ARM1176JZF-S",
        board: "raspberry-pi-model-b-plus-v1.2",
        rust_target: TARGET,
        boot_path: "raspberry-pi-videocore-firmware-direct-kernel",
        entry_symbol: ENTRY,
        entry_address: facts.entry,
        load_address: LOAD_ADDRESS,
        elf_class: "ELF32",
        byte_order: "little-endian",
        eabi_version: facts.eabi_version,
        hard_float_abi: facts.hard_float,
        hosted_interpreter: false,
        dynamic_linkage: false,
        runtime_bases_available: false,
        boot_claimed: false,
        elf_sha256: elf_digest.clone(),
        kernel_image_sha256: image_digest,
        kernel_image_bytes: image.len(),
    };
    fs::write(
        paths.target.join("a0-inspection.json"),
        serde_json::to_vec_pretty(&inspection)
            .map_err(|error| refusal("build-record-failed", error))?,
    )
    .map_err(|error| refusal("build-record-failed", error))?;
    let record = record(&paths, elf_digest)?;
    fs::write(
        paths.target.join("build.json"),
        serde_json::to_vec_pretty(&record)
            .map_err(|error| refusal("build-record-failed", error))?,
    )
    .map_err(|error| refusal("build-record-failed", error))?;
    if !opts.quiet && !opts.json {
        println!("ConduitOS ARMv6 ELF: {}", paths.kernel.display());
        println!("Raspberry Pi kernel image: {}", kernel_image.display());
    }
    Ok(record)
}

#[derive(Debug)]
struct ElfFacts {
    entry: u32,
    eabi_version: u8,
    hard_float: bool,
    load_segments: Vec<LoadSegment>,
}

#[derive(Debug)]
struct LoadSegment {
    offset: usize,
    address: u32,
    file_bytes: usize,
    memory_bytes: usize,
}

fn inspect_elf(bytes: &[u8]) -> Result<ElfFacts, ConduitosError> {
    if bytes.len() < ELF_HEADER_BYTES
        || &bytes[..4] != b"\x7fELF"
        || bytes[4] != 1
        || bytes[5] != 1
        || bytes[6] != 1
    {
        return Err(invalid("expected a little-endian ELF32 artifact"));
    }
    if u16_at(bytes, 16)? != ET_EXEC || u16_at(bytes, 18)? != EM_ARM {
        return Err(invalid("artifact is not an executable ARM ELF"));
    }
    let entry = u32_at(bytes, 24)?;
    if entry != LOAD_ADDRESS {
        return Err(invalid(format!(
            "entry is {entry:#x}; expected {LOAD_ADDRESS:#x}"
        )));
    }
    let flags = u32_at(bytes, 36)?;
    let eabi_version = (flags >> 24) as u8;
    let hard_float = flags & 0x400 != 0;
    if eabi_version != 5 || hard_float {
        return Err(invalid("expected ARM EABI5 with the soft-float ABI"));
    }
    let program_offset = u32_at(bytes, 28)? as usize;
    let program_size = u16_at(bytes, 42)? as usize;
    let program_count = u16_at(bytes, 44)? as usize;
    if program_size != PROGRAM_HEADER_BYTES {
        return Err(invalid("unexpected ELF32 program-header size"));
    }
    let mut load_segments = Vec::new();
    for index in 0..program_count {
        let offset = program_offset
            .checked_add(
                index
                    .checked_mul(program_size)
                    .ok_or_else(|| invalid("program headers overflow"))?,
            )
            .ok_or_else(|| invalid("program headers overflow"))?;
        if u32_at(bytes, offset)? != PT_LOAD {
            continue;
        }
        let segment = LoadSegment {
            offset: u32_at(bytes, offset + 4)? as usize,
            address: u32_at(bytes, offset + 12)?,
            file_bytes: u32_at(bytes, offset + 16)? as usize,
            memory_bytes: u32_at(bytes, offset + 20)? as usize,
        };
        if segment.address >= LOAD_ADDRESS {
            load_segments.push(segment);
        }
    }
    if load_segments.is_empty() || load_segments[0].address != LOAD_ADDRESS {
        return Err(invalid(
            "no load segment begins at the Raspberry Pi load address",
        ));
    }
    Ok(ElfFacts {
        entry,
        eabi_version,
        hard_float,
        load_segments,
    })
}

fn flatten_load_image(bytes: &[u8], segments: &[LoadSegment]) -> Result<Vec<u8>, ConduitosError> {
    let end = segments.iter().try_fold(LOAD_ADDRESS, |end, segment| {
        let segment_end = segment
            .address
            .checked_add(
                segment
                    .memory_bytes
                    .try_into()
                    .map_err(|_| invalid("segment exceeds ARM address space"))?,
            )
            .ok_or_else(|| invalid("segment exceeds ARM address space"))?;
        Ok::<_, ConduitosError>(end.max(segment_end))
    })?;
    let mut image = vec![0_u8; (end - LOAD_ADDRESS) as usize];
    for segment in segments {
        let source_end = segment
            .offset
            .checked_add(segment.file_bytes)
            .ok_or_else(|| invalid("segment file range overflow"))?;
        let source = bytes
            .get(segment.offset..source_end)
            .ok_or_else(|| invalid("segment exceeds ELF bytes"))?;
        let destination = (segment.address - LOAD_ADDRESS) as usize;
        image[destination..destination + source.len()].copy_from_slice(source);
    }
    Ok(image)
}

fn inspect_symbol(elf: &Path, root: &Path) -> Result<(), ConduitosError> {
    let output = Command::new("readelf")
        .args(["-sW"])
        .arg(elf)
        .current_dir(root)
        .output()
        .map_err(|error| refusal("readelf-unavailable", error))?;
    if !output.status.success() {
        return Err(refusal("readelf-unavailable", output.status));
    }
    let symbols = String::from_utf8_lossy(&output.stdout);
    if !symbols
        .lines()
        .any(|line| line.contains(ENTRY) && line.contains("00008000"))
    {
        return Err(invalid("exact ARMv6 entry symbol is absent or misplaced"));
    }
    Ok(())
}

fn record(paths: &Paths, digest: String) -> Result<BuildRecord, ConduitosError> {
    Ok(BuildRecord {
        schema: "conduit.conduitos.build/v1",
        base_commit: git_head(&paths.root)?,
        architecture: "armv6",
        rust_target: TARGET,
        limine_crate: "none",
        elf_sha256: digest,
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
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn invalid(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("invalid-armv6-rpi-b-plus-a0-artifact", detail)
}

fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_foreign_and_truncated_artifacts() {
        assert!(inspect_elf(&[]).is_err());
        let mut header = vec![0_u8; ELF_HEADER_BYTES];
        header[..7].copy_from_slice(b"\x7fELF\x01\x01\x01");
        header[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
        header[18..20].copy_from_slice(&62_u16.to_le_bytes());
        assert!(inspect_elf(&header).is_err());
    }

    #[test]
    fn flattening_rejects_out_of_bounds_segment_bytes() {
        let segment = LoadSegment {
            offset: 4,
            address: LOAD_ADDRESS,
            file_bytes: 2,
            memory_bytes: 2,
        };
        assert!(flatten_load_image(&[0; 5], &[segment]).is_err());
    }
}
