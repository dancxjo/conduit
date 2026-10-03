//! Narrow entry-prologue evidence, never a complete stack/call-chain bound.
use std::{fs, path::Path, process::Command};
#[path = "native_speech_inventory.rs"]
mod inventory;

pub(super) fn inspect(
    root: &Path,
    artifact: &Path,
    evidence: &Path,
    binary: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let libdir = Command::new("rustc")
        .args(["--print", "target-libdir"])
        .current_dir(root)
        .output()?;
    if !libdir.status.success() {
        return Err("cannot resolve the pinned Rust toolchain for stack inspection".into());
    }
    let libdir = std::path::PathBuf::from(std::str::from_utf8(&libdir.stdout)?.trim());
    let tool = libdir
        .parent()
        .ok_or("Rust target libdir has no parent")?
        .join("bin")
        .join(if cfg!(windows) {
            "llvm-objdump.exe"
        } else {
            "llvm-objdump"
        });
    if !tool.is_file() {
        return Err(
            "Cortex entry-prologue inspection requires the pinned Rust llvm-tools component".into(),
        );
    }
    let output = Command::new(tool)
        .args(["-d", "--syms", "--no-show-raw-insn", "--no-print-imm-hex"])
        .arg(artifact)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "entry disassembly failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let assembly = std::str::from_utf8(&output.stdout)?;
    fs::create_dir_all(evidence)?;
    let path = evidence.join(format!("{binary}.entry.asm"));
    // Preserve the existing entry artifact and retain the complete linked code
    // separately, so downstream inspection never relies on a truncated view.
    let entry = assembly
        .lines()
        .skip_while(|line| !line.trim().ends_with(" <_start>:"))
        .take_while(|line| line.trim().ends_with(" <_start>:") || !line.trim().ends_with(">:"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &path,
        format!("probe: file format elf32-littlearm\n{entry}\n"),
    )?;
    let linked = evidence.join(format!("{binary}.linked.asm"));
    fs::write(&linked, assembly)?;
    let report = inventory::report(assembly)?;
    let inventory = evidence.join(format!("{binary}.stack-inventory.json"));
    fs::write(&inventory, serde_json::to_vec_pretty(&report)?)?;
    println!("Linked stack inventory: {}. Full stack remains unproven; {} identified computed-control sites. Retained {}",inventory.display(),report["identified_computed_control_sites"].as_array().map_or(0, Vec::len),linked.display());
    let bytes = entry_prologue(assembly).map_err(|error| format!("{}: {error}", path.display()))?;
    println!("Entry _start prologue: {bytes} bytes (saved registers plus fixed SP subtraction). Retained {}", path.display());
    println!("Entry contribution only: callee frames, body stack changes, boot and interrupts are excluded. This is a stack lower bound, not total stack or device-fit acceptance.");
    Ok(())
}

fn immediate(text: &str) -> Result<usize, &'static str> {
    let value = text
        .trim()
        .strip_prefix('#')
        .ok_or("stack adjustment lacks an immediate")?
        .parse::<usize>()
        .map_err(|_| "stack adjustment is not a nonnegative decimal immediate")?;
    if value == 0 || value % 4 != 0 {
        return Err("stack subtraction must be positive and word-aligned");
    }
    Ok(value)
}

fn registers(text: &str) -> Result<usize, &'static str> {
    let contents = text
        .trim()
        .strip_prefix('{')
        .and_then(|text| text.strip_suffix('}'))
        .ok_or("push lacks an exact register list")?;
    let mut seen = std::collections::BTreeSet::new();
    for register in contents.split(',').map(str::trim) {
        let valid = register == "lr"
            || register
                .strip_prefix('r')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| n <= 12 && register.len() == if n < 10 { 2 } else { 3 });
        if !valid || !seen.insert(register) {
            return Err("push has an unknown, ranged or duplicate register");
        }
    }
    seen.len().checked_mul(4).ok_or("register frame overflow")
}

pub(super) fn entry_prologue(assembly: &str) -> Result<usize, &'static str> {
    if !assembly.contains("file format elf32-littlearm") {
        return Err("entry inspection supports only the linked 32-bit ARM ELF probe");
    }
    let mut lines = assembly
        .lines()
        .skip_while(|line| !line.trim().ends_with(" <_start>:"));
    lines.next().ok_or("entry symbol _start is absent")?;
    let mut bytes = 0usize;
    let mut pushed = false;
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (address, instruction) = line
            .trim()
            .split_once(':')
            .ok_or("malformed entry instruction")?;
        u64::from_str_radix(address, 16).map_err(|_| "entry address is not hexadecimal")?;
        let instruction = instruction.trim();
        let split = instruction
            .find(char::is_whitespace)
            .unwrap_or(instruction.len());
        let (opcode, operands) = instruction.split_at(split);
        let operands = operands.trim();
        let addition = match opcode {
            "push" => {
                pushed = true;
                registers(operands)?
            }
            "add" if operands.starts_with("r7, sp, #") && pushed => {
                let offset = operands[9..]
                    .parse::<usize>()
                    .map_err(|_| "frame-pointer offset is not decimal")?;
                if offset % 4 != 0 || offset > bytes {
                    return Err("frame-pointer offset is outside saved registers");
                }
                0
            }
            "sub" if operands.starts_with("sp,") && pushed => immediate(&operands[3..])?,
            _ if !pushed => return Err("entry lacks a supported register-save prologue"),
            _ if operands.starts_with("r7, sp") => return Err("unsupported frame-pointer setup"),
            _ if operands.starts_with("sp,") => {
                return Err("entry has an unsupported stack adjustment")
            }
            _ => return Ok(bytes),
        };
        bytes = bytes.checked_add(addition).ok_or("entry frame overflow")?;
    }
    Err("entry prologue has no following body instruction")
}

#[cfg(test)]
mod tests {
    use super::entry_prologue;
    const HEADER: &str = "probe: file format elf32-littlearm\n10000000 <_start>:\n";
    #[test]
    fn saves_and_split_stack_subtractions_are_counted_once() {
        let assembly = format!("{HEADER}10000000: push {{r4, r5, r6, r7, lr}}\n10000002: add r7, sp, #12\n10000004: sub sp, #508\n10000006: sub sp, #508\n10000008: sub sp, #36\n1000000a: add r0, sp, #660\n1000000c: bl 0x10000100 <callee>\n");
        assert_eq!(entry_prologue(&assembly), Ok(1072));
    }
    #[test]
    fn unknown_architecture_entry_and_stack_operations_refuse() {
        for body in [
            "movs r0, #0",
            "push {r4-r7, lr}",
            "push {r4, r4, lr}",
            "push {r01, lr}",
            "push {r4, lr}\n10000002: add r7, sp, #13",
            "push {r4, lr}\n10000002: mov r7, sp",
            "push {r4, lr}\n10000002: sub sp, r0",
            "push {r4, lr}\n10000002: sub sp, #3",
            "push {r4, lr}\n10000002: mov sp, r0",
        ] {
            assert!(entry_prologue(&format!("{HEADER}10000000: {body}\n")).is_err());
        }
        assert!(entry_prologue("probe: file format elf64-x86-64").is_err());
        assert!(entry_prologue("probe: file format elf32-littlearm").is_err());
    }
}
