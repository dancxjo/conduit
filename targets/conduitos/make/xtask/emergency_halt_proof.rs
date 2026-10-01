//! External witness for the x86_64 emergency halt callback.

use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{image, profile::Paths, qmp, report::sha256_file, ConduitosArch, ConduitosError};

const SIGN: &str = "CONDUIT_EMERGENCY_HALT_SIGN ";

#[derive(Serialize)]
struct EmergencyHaltProof {
    schema: &'static str,
    status: &'static str,
    proof_class: &'static str,
    architecture: &'static str,
    operation: &'static str,
    request_sign_count: usize,
    cpu_halted: bool,
    register_state_stable: bool,
    interrupts_disabled: bool,
    preceding_opcode_hlt: bool,
    qemu_process_alive: bool,
    serial_quiescent_after_request: bool,
    iso_sha256: String,
}

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-emergency-halt-proof",
            "emergency halt proof requires an externally observed guest CPU halt",
        ));
    }
    image::execute_emergency_halt(opts)?;
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let monitor_socket = paths.target.join("emergency-halt-monitor.sock");
    let serial_path = paths.target.join("emergency-halt-serial.log");
    let _ = fs::remove_file(&monitor_socket);
    let _ = fs::remove_file(&serial_path);
    let monitor = format!("unix:{},server=on,wait=off", monitor_socket.display());
    let serial = format!("file:{}", serial_path.display());
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-M", "q35", "-cpu", "max", "-m", "64M", "-smp", "1", "-display", "none", "-monitor",
            "none", "-qmp", &monitor, "-serial", &serial, "-net", "none", "-cdrom",
        ])
        .arg(&paths.iso)
        .args(["-boot", "d"])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    let result = observe(&paths, &monitor_socket, &serial_path, &mut child);
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(&monitor_socket);
    result
}

fn observe(
    paths: &Paths,
    monitor_socket: &std::path::Path,
    serial_path: &std::path::Path,
    child: &mut std::process::Child,
) -> Result<(), ConduitosError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let serial = loop {
        let bytes = fs::read(serial_path).unwrap_or_default();
        if String::from_utf8_lossy(&bytes).contains(SIGN) {
            break bytes;
        }
        if child.try_wait().map_err(io)?.is_some() || Instant::now() >= deadline {
            return Err(ConduitosError::refusal(
                "emergency-halt-sign-timeout",
                "guest did not emit the bounded halt request sign",
            ));
        }
        thread::sleep(Duration::from_millis(20));
    };
    thread::sleep(Duration::from_millis(200));
    let after = fs::read(serial_path).map_err(io)?;
    let (mut stream, mut reader) = qmp::connect(monitor_socket, child)?;
    let registers_before = qmp::request_value(
        &mut stream,
        &mut reader,
        br#"{"execute":"human-monitor-command","arguments":{"command-line":"info registers"}}"#,
        "emergency-halt-registers-before",
    )?;
    thread::sleep(Duration::from_millis(100));
    let registers_after = qmp::request_value(
        &mut stream,
        &mut reader,
        br#"{"execute":"human-monitor-command","arguments":{"command-line":"info registers"}}"#,
        "emergency-halt-registers-after",
    )?;
    let registers = registers_before.as_str().ok_or_else(|| {
        ConduitosError::refusal("emergency-halt-registers-invalid", "expected HMP text")
    })?;
    let (rip, interrupts_disabled) = parse_machine_state(registers)?;
    let instruction_command = serde_json::to_vec(&serde_json::json!({
        "execute": "human-monitor-command",
        "arguments": {"command-line": format!("x/1bx 0x{:x}", rip - 1)},
    }))
    .map_err(|error| {
        ConduitosError::refusal("emergency-halt-command-invalid", error.to_string())
    })?;
    let instruction = qmp::request_value(
        &mut stream,
        &mut reader,
        &instruction_command,
        "emergency-halt-instruction",
    )?;
    let register_state_stable = registers_before == registers_after;
    let preceding_opcode_hlt = instruction
        .as_str()
        .is_some_and(|state| state.to_ascii_lowercase().contains("0xf4"));
    let cpu_halted = register_state_stable && interrupts_disabled && preceding_opcode_hlt;
    let process_alive = child.try_wait().map_err(io)?.is_none();
    let sign_count = String::from_utf8_lossy(&after).matches(SIGN).count();
    if !cpu_halted || !process_alive || serial != after || sign_count != 1 {
        return Err(ConduitosError::refusal(
            "emergency-halt-observation-invalid",
            format!(
                "cpu_halted={cpu_halted} instruction={instruction} process_alive={process_alive} serial_quiescent={} sign_count={sign_count} registers_before={registers_before} registers_after={registers_after}",
                serial == after,
            ),
        ));
    }
    let proof = EmergencyHaltProof {
        schema: "conduit.conduitos/emergency-halt-proof@1",
        status: "completed",
        proof_class: "freestanding-emulator",
        architecture: "x86_64",
        operation: "conduitos.machine/halt@1",
        request_sign_count: sign_count,
        cpu_halted,
        register_state_stable,
        interrupts_disabled,
        preceding_opcode_hlt,
        qemu_process_alive: process_alive,
        serial_quiescent_after_request: true,
        iso_sha256: sha256_file(&paths.iso)?,
    };
    let mut bytes = serde_json::to_vec_pretty(&proof).map_err(|error| {
        ConduitosError::refusal("emergency-halt-proof-invalid", error.to_string())
    })?;
    bytes.push(b'\n');
    fs::write(&paths.emergency_halt_proof, bytes).map_err(io)?;
    println!(
        "ConduitOS emergency halt proof: {}",
        paths.emergency_halt_proof.display()
    );
    Ok(())
}

fn io(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("emergency-halt-proof-io", error.to_string())
}

fn parse_machine_state(registers: &str) -> Result<(u64, bool), ConduitosError> {
    let rip = parse_register(registers, "RIP=")?;
    let flags = parse_register(registers, "RFL=")?;
    Ok((rip, flags & (1 << 9) == 0))
}

fn parse_register(registers: &str, label: &str) -> Result<u64, ConduitosError> {
    let value = registers
        .split_whitespace()
        .find_map(|field| field.strip_prefix(label))
        .ok_or_else(|| {
            ConduitosError::refusal(
                "emergency-halt-registers-invalid",
                format!("missing {label}"),
            )
        })?;
    u64::from_str_radix(value, 16).map_err(|error| {
        ConduitosError::refusal("emergency-halt-registers-invalid", error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_state_requires_exact_hex_registers_and_reports_interrupt_mask() {
        assert_eq!(
            parse_machine_state("RIP=ffffffff80000101 RFL=00000046").unwrap(),
            (0xffff_ffff_8000_0101, true)
        );
        assert_eq!(
            parse_machine_state("RIP=10 RFL=246").unwrap(),
            (0x10, false)
        );
        assert!(parse_machine_state("RIP=10").is_err());
    }
}
