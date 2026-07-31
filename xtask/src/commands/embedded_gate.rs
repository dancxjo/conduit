use std::{fs, path::Path, process::Command};

use serde_json::{Value, json};
use sha2::Digest;

pub fn current_firmware_identity(
    workspace_root: &Path,
    target: &str,
    profile: &str,
) -> Result<Vec<u8>, String> {
    let firmware_root = workspace_root.join("firmware/conduit-rp2040-hil");
    let firmware_inputs = [
        "../../Cargo.lock",
        "../../Cargo.toml",
        "../../crates/conduit-core/Cargo.toml",
        "../../crates/conduit-core/src",
        "../../crates/conduit-embedded/Cargo.toml",
        "../../crates/conduit-embedded/src/lib.rs",
        "Cargo.toml",
        "build.rs",
        "memory.x",
        "src/lib.rs",
        "src/main.rs",
    ];

    let mut hasher = sha2::Sha256::new();

    for relative in firmware_inputs {
        let source = firmware_root.join(relative);
        let mut inputs: Vec<(String, std::path::PathBuf)> = Vec::new();
        if source.is_dir() {
            fn collect_files(
                base: &Path,
                current: &Path,
                inputs: &mut Vec<(String, std::path::PathBuf)>,
                rel_prefix: &str,
            ) {
                if let Ok(entries) = fs::read_dir(current) {
                    let mut sorted_entries: Vec<_> = entries.flatten().collect();
                    sorted_entries.sort_by_key(|e| e.path());
                    for entry in sorted_entries {
                        let path = entry.path();
                        if path.is_dir() {
                            collect_files(base, &path, inputs, rel_prefix);
                        } else if path.is_file() {
                            let rel = path
                                .strip_prefix(base)
                                .unwrap()
                                .to_string_lossy()
                                .to_string();
                            let key = format!("{rel_prefix}/{rel}");
                            inputs.push((key, path));
                        }
                    }
                }
            }
            collect_files(&source, &source, &mut inputs, relative);
        } else if source.is_file() {
            inputs.push((relative.to_string(), source));
        }

        inputs.sort_by(|a, b| a.0.cmp(&b.0));

        for (label, path) in inputs {
            let content = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            Digest::update(&mut hasher, label.as_bytes());
            Digest::update(&mut hasher, b"\0");
            Digest::update(&mut hasher, (content.len() as u64).to_be_bytes());
            Digest::update(&mut hasher, &content);
        }
    }

    let rustc_vv = Command::new("rustc")
        .arg("-vV")
        .current_dir(workspace_root)
        .output()
        .map_err(|e| format!("rustc exec: {e}"))?
        .stdout;

    for (label, content) in [
        ("cargo-target", target.as_bytes()),
        ("cargo-profile", profile.as_bytes()),
        ("rustc-version", rustc_vv.as_slice()),
    ] {
        Digest::update(&mut hasher, label.as_bytes());
        Digest::update(&mut hasher, b"\0");
        Digest::update(&mut hasher, (content.len() as u64).to_be_bytes());
        Digest::update(&mut hasher, content);
    }

    Ok(Digest::finalize(hasher).to_vec())
}

pub fn run(workspace_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let budget_path = workspace_root.join("conformance/c5/rp2040-budgets.json");
    if !budget_path.exists() {
        return Err(format!("Budget file missing: {}", budget_path.display()).into());
    }

    let budget: Value = serde_json::from_str(&fs::read_to_string(&budget_path)?)?;
    let target = budget
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or("thumbv6m-none-eabi");
    let artifact_rel = budget.get("artifact").and_then(Value::as_str).unwrap_or("");

    let build_status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "conduit-rp2040-hil",
            "--target",
            target,
            "--release",
        ])
        .current_dir(workspace_root)
        .status()?;

    if !build_status.success() {
        return Err("cargo build conduit-rp2040-hil failed".into());
    }

    let artifact = workspace_root.join(artifact_rel);
    if !artifact.exists() {
        return Err(format!("Built firmware missing: {}", artifact.display()).into());
    }

    // Run size
    let size_out = Command::new("size")
        .arg(&artifact)
        .current_dir(workspace_root)
        .output()?;

    if !size_out.status.success() {
        return Err(format!("size failed: {}", String::from_utf8_lossy(&size_out.stderr)).into());
    }

    let size_text = String::from_utf8_lossy(&size_out.stdout);
    let lines: Vec<&str> = size_text.trim().lines().collect();
    if lines.len() != 2 {
        return Err("unexpected size output format".into());
    }
    let parts: Vec<&str> = lines[1].split_whitespace().collect();
    if parts.len() < 3 {
        return Err("unexpected size output fields".into());
    }

    let text_bytes: u64 = parts[0].parse()?;
    let data_bytes: u64 = parts[1].parse()?;
    let bss_bytes: u64 = parts[2].parse()?;

    let flash_bytes = text_bytes + data_bytes;
    let static_ram_bytes = data_bytes + bss_bytes;

    let max_flash = budget
        .get("maximum_flash_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let max_static_ram = budget
        .get("maximum_static_ram_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    if flash_bytes > max_flash {
        return Err(format!("RP2040 flash budget exceeded: {flash_bytes} > {max_flash}").into());
    }
    if static_ram_bytes > max_static_ram {
        return Err(format!(
            "RP2040 static RAM budget exceeded: {static_ram_bytes} > {max_static_ram}"
        )
        .into());
    }

    // Check undefined symbols
    let nm_out = Command::new("nm")
        .args(["-u", artifact.to_str().unwrap()])
        .current_dir(workspace_root)
        .output()?;
    if !nm_out.status.success() {
        return Err(format!("nm failed: {}", String::from_utf8_lossy(&nm_out.stderr)).into());
    }

    let nm_text = String::from_utf8_lossy(&nm_out.stdout);
    let forbidden_symbols = budget
        .get("allocator_symbols_forbidden")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut forbidden_detected = Vec::new();
    for sym_val in &forbidden_symbols {
        if let Some(sym) = sym_val.as_str() {
            if nm_text.lines().any(|l| l.contains(sym)) {
                forbidden_detected.push(sym);
            }
        }
    }
    if !forbidden_detected.is_empty() {
        return Err(format!(
            "allocator linkage detected: {}",
            forbidden_detected.join(", ")
        )
        .into());
    }

    // Read ELF header
    let elf_h_out = Command::new("readelf")
        .args(["-h", artifact.to_str().unwrap()])
        .current_dir(workspace_root)
        .output()?;
    let elf_h_text = String::from_utf8_lossy(&elf_h_out.stdout);
    if !elf_h_text.contains("Machine:                           ARM") {
        return Err("firmware artifact is not an ARM ELF".into());
    }

    // Read ELF sections
    let elf_s_out = Command::new("readelf")
        .args(["-W", "-S", artifact.to_str().unwrap()])
        .current_dir(workspace_root)
        .output()?;
    let elf_s_text = String::from_utf8_lossy(&elf_s_out.stdout);

    // Find .boot2 line
    let mut boot2_found = false;
    let mut boot2_addr: u64 = 0;
    let mut boot2_sz: u64 = 0;

    for line in elf_s_text.lines() {
        if line.contains(".boot2") {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let boot2_idx = tokens.iter().position(|&t| t == ".boot2");
            if let Some(idx) = boot2_idx {
                if tokens.len() > idx + 4 {
                    if let (Ok(addr), Ok(size)) = (
                        u64::from_str_radix(tokens[idx + 2], 16),
                        u64::from_str_radix(tokens[idx + 4], 16),
                    ) {
                        boot2_found = true;
                        boot2_addr = addr;
                        boot2_sz = size;
                    }
                }
            }
        }
    }

    if !boot2_found {
        return Err("firmware artifact omits the RP2040 .boot2 section".into());
    }
    if boot2_addr != 0x10000000 || boot2_sz != 0x100 {
        return Err(format!(
            "RP2040 .boot2 must occupy exactly 0x10000000..0x10000100, got address=0x{boot2_addr:08x} size=0x{boot2_sz:x}"
        ).into());
    }

    let firmware_id = current_firmware_identity(workspace_root, target, "release")?;

    let report = json!({
        "schema": "conduit.rp2040-budget-report",
        "target": target,
        "artifact": artifact_rel,
        "firmware_identity": format!("sha256:{}", hex::encode(firmware_id)),
        "flash": {
            "bytes": flash_bytes,
            "maximum_bytes": max_flash,
            "kind": "linked-elf-load-image",
        },
        "static_ram": {
            "bytes": static_ram_bytes,
            "maximum_bytes": max_static_ram,
            "kind": "linked-elf-data-plus-bss",
        },
        "stack": {
            "bytes": budget.get("declared_stack_budget_bytes"),
            "kind": "profile-declared-reviewed-ceiling-not-elf-measurement",
        },
        "boot2": {
            "address": format!("0x{boot2_addr:08x}"),
            "bytes": boot2_sz,
            "kind": "linked-rp2040-second-stage-bootloader",
        },
        "allocator_undefined_symbols": [],
    });

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
