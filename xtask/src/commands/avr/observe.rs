//! One guarded physical execution of an ordinary Conduit contact Form.

use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use clap::Args;
use conduit_core::{
    decode_assigned_execution_receipt, AssignedIdentity, AssignedTerminalDisposition,
    ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES,
};
use serde::Serialize;

use super::{
    plan::plan_contact, require_success, run_build, upload_artifact, write_receipt, PhysicalGate,
};
use crate::{cli::GlobalOpts, workspace::workspace_root};

const APPLICATION_VID: &str = "1b4f";
const APPLICATION_PID: &str = "9206";
const MAX_RECEIPT_BYTES: usize = ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES + 1;
const BOOTLOADER_WAIT: Duration = Duration::from_secs(300);

#[derive(Args, Debug)]
pub(super) struct ObserveContactArgs {
    /// Accepted physical receive-only receipt produced by `avr receive-only`.
    #[arg(long)]
    rx_proof: PathBuf,
    #[arg(long)]
    create_stopped: bool,
    #[arg(long)]
    attended: bool,
    #[arg(long)]
    wheels_clear: bool,
    #[arg(long)]
    common_ground_verified: bool,
    #[arg(long)]
    rx_voltage_compatible: bool,
    #[arg(
        long,
        default_value = "target/avr-promicro/contact-observation-receipt.json"
    )]
    receipt: PathBuf,
}

#[derive(Debug, Serialize)]
struct ContactObservationReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    artifact_sha256: String,
    rx_proof: String,
    boot_id: String,
    plan_identity: String,
    fragment_identity: String,
    active_play_identity: String,
    output_port: u16,
    terminal_disposition: &'static str,
    terminal_detail: u16,
    value_bytes: usize,
    contact_body_sectors: Option<u8>,
    create_stopped: bool,
    attended: bool,
    wheels_clear: bool,
    common_ground_verified: bool,
    rx_voltage_compatible: bool,
}

pub(super) fn run(
    args: ObserveContactArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_request(&args)?;
    let root = workspace_root()?;
    if opts.dry_run {
        if !opts.quiet {
            println!("would verify accepted receive-only proof, require one exact 2341:0036 Caterina bootloader, build and flash the Rust AVR Host, derive its Boot identity from 1b4f:9206 USB, plan the ordinary contact Form, and execute one bounded assigned Plan");
        }
        return Ok(());
    }
    validate_rx_proof(&root.join(&args.rx_proof))?;

    let built = run_build(Path::new("target/avr-promicro/build-receipt.json"), opts)?;
    let upload_port = super::rx_check::wait_for_bootloader_port(BOOTLOADER_WAIT)?;
    upload_artifact(
        &root,
        &upload_port,
        &built.path,
        "guarded assigned-Plan AVR Host flash",
    )?;

    let runtime = wait_for_runtime(Duration::from_secs(15))?;
    configure_serial(&runtime.path)?;
    let planned = plan_contact(&runtime.boot_id)?;
    let mut device = open_nonblocking(&runtime.path)?;
    write_chunks(&mut device, &planned.assigned, Duration::from_secs(2))?;
    write_chunks(&mut device, &planned.activation, Duration::from_secs(2))?;
    let encoded = read_receipt(&mut device, Duration::from_secs(4))?;
    let decoded = decode_assigned_execution_receipt(&encoded)
        .map_err(|error| format!("AVR terminal receipt refused: {error:?}"))?;
    if decoded.activation.plan != planned.plan
        || decoded.activation.fragment != planned.fragment
        || decoded.activation.host != planned.host
        || decoded.activation.boot != planned.boot
        || decoded.activation.active_play != planned.active_play
    {
        return Err("AVR terminal receipt identities did not match the planned execution".into());
    }

    let completed = decoded.output_port == planned.expected_output_port
        && decoded.disposition == AssignedTerminalDisposition::Completed
        && decoded.detail == 0
        && decoded.value.len() == planned.expected_value_bytes;
    let terminal_disposition = disposition_name(decoded.disposition);

    let record = ContactObservationReceipt {
        schema: "conduit.avr-promicro/contact-observation@2",
        outcome: if completed { "completed" } else { "terminal" },
        proof_class: "physical-create-oi-assigned-plan",
        artifact_sha256: built.artifact_sha256,
        rx_proof: args.rx_proof.display().to_string(),
        boot_id: runtime.boot_id,
        plan_identity: identity_hex(planned.plan),
        fragment_identity: identity_hex(planned.fragment),
        active_play_identity: identity_hex(planned.active_play),
        output_port: decoded.output_port,
        terminal_disposition,
        terminal_detail: decoded.detail,
        value_bytes: decoded.value.len(),
        contact_body_sectors: decoded.value.first().copied(),
        create_stopped: args.create_stopped,
        attended: args.attended,
        wheels_clear: args.wheels_clear,
        common_ground_verified: args.common_ground_verified,
        rx_voltage_compatible: args.rx_voltage_compatible,
    };
    let receipt_path = root.join(args.receipt);
    write_receipt(&receipt_path, &record, opts)?;
    if !completed {
        return Err(format!(
            "AVR Host returned terminal disposition={terminal_disposition} detail={} output_port={} value_bytes={}; wrote {}",
            decoded.detail,
            decoded.output_port,
            decoded.value.len(),
            receipt_path.display()
        )
        .into());
    }
    Ok(())
}

fn disposition_name(disposition: AssignedTerminalDisposition) -> &'static str {
    match disposition {
        AssignedTerminalDisposition::Completed => "completed",
        AssignedTerminalDisposition::Refused => "refused",
        AssignedTerminalDisposition::Failed => "failed",
        AssignedTerminalDisposition::Cancelled => "cancelled",
    }
}

fn validate_request(args: &ObserveContactArgs) -> Result<(), Box<dyn std::error::Error>> {
    PhysicalGate {
        create_stopped: args.create_stopped,
        attended: args.attended,
        wheels_clear: args.wheels_clear,
    }
    .validate("contact observation")?;
    if !args.common_ground_verified || !args.rx_voltage_compatible {
        return Err(
            "AVR contact observation requires --common-ground-verified --rx-voltage-compatible"
                .into(),
        );
    }
    Ok(())
}

pub(super) fn validate_rx_proof(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let proof: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    let exact = proof.get("schema").and_then(|value| value.as_str())
        == Some("conduit.avr-promicro/create-rx-check@2")
        && proof.get("outcome").and_then(|value| value.as_str()) == Some("stable-high")
        && proof.get("proof_class").and_then(|value| value.as_str())
            == Some("physical-gpio-receive-only")
        && proof.get("samples").and_then(|value| value.as_u64()) == Some(2_048)
        && proof.get("high_samples").and_then(|value| value.as_u64()) == Some(2_048)
        && proof.get("low_samples").and_then(|value| value.as_u64()) == Some(0)
        && proof.get("transitions").and_then(|value| value.as_u64()) == Some(0)
        && proof.get("create_uart").and_then(|value| value.as_str())
            == Some("isolated-no-transmitter")
        && [
            "create_stopped",
            "attended",
            "wheels_clear",
            "d1_disconnected_or_high_impedance",
            "common_ground_verified",
            "rx_voltage_compatible",
        ]
        .into_iter()
        .all(|key| proof.get(key).and_then(|value| value.as_bool()) == Some(true));
    if !exact {
        return Err(
            "AVR contact observation requires an accepted stable-high #1965 receive-only receipt"
                .into(),
        );
    }
    Ok(())
}

pub(super) struct RuntimeDevice {
    pub(super) path: PathBuf,
    pub(super) boot_id: String,
}

pub(super) fn wait_for_runtime(
    timeout: Duration,
) -> Result<RuntimeDevice, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let matches = runtime_devices()?;
        if let [runtime] = matches.as_slice() {
            return Ok(RuntimeDevice {
                path: runtime.path.clone(),
                boot_id: runtime.boot_id.clone(),
            });
        }
        if matches.len() > 1 {
            return Err("multiple AVR Host runtime devices are present".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("AVR Host did not re-enumerate with one observed Boot identity".into())
}

fn runtime_devices() -> Result<Vec<RuntimeDevice>, Box<dyn std::error::Error>> {
    let mut matches = Vec::new();
    for entry in fs::read_dir("/dev")? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.strip_prefix("ttyACM").is_some_and(|index| {
            !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
        }) {
            continue;
        }
        let output = Command::new("udevadm")
            .args(["info", "--query=property", "--name"])
            .arg(&path)
            .output()?;
        if !output.status.success() {
            continue;
        }
        let properties = String::from_utf8(output.stdout)?;
        let boot_id = property(&properties, "ID_SERIAL_SHORT");
        if property(&properties, "ID_VENDOR_ID") == Some(APPLICATION_VID)
            && property(&properties, "ID_MODEL_ID") == Some(APPLICATION_PID)
            && boot_id.is_some_and(valid_boot_id)
        {
            matches.push(RuntimeDevice {
                path,
                boot_id: boot_id.unwrap().to_owned(),
            });
        }
    }
    Ok(matches)
}

fn property<'a>(properties: &'a str, key: &str) -> Option<&'a str> {
    properties
        .lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
}

fn valid_boot_id(value: &str) -> bool {
    value.len() == 12
        && value.starts_with("avr-")
        && value[4..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn configure_serial(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("stty")
        .arg("-F")
        .arg(path)
        .args(["raw", "-echo", "115200"])
        .output()?;
    require_success(&output, "AVR Host CDC configuration")
}

pub(super) fn open_nonblocking(path: &Path) -> Result<File, Box<dyn std::error::Error>> {
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?)
}

fn write_chunks(
    device: &mut File,
    bytes: &[u8],
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    let mut offset = 0;
    while offset < bytes.len() {
        if !super::rx_check::wait_ready(device, libc::POLLOUT, deadline)? {
            return Err("timed out writing assigned AVR Host frame".into());
        }
        let end = (offset + 32).min(bytes.len());
        match device.write(&bytes[offset..end]) {
            Ok(0) => {}
            Ok(written) => offset += written,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn read_receipt(
    device: &mut File,
    timeout: Duration,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    let mut bytes = vec![0_u8; MAX_RECEIPT_BYTES];
    let mut offset = 0;
    let mut expected = None;
    loop {
        if !super::rx_check::wait_ready(device, libc::POLLIN, deadline)? {
            return Err("timed out reading AVR Host terminal receipt".into());
        }
        let end = expected.unwrap_or(bytes.len());
        match device.read(&mut bytes[offset..end]) {
            Ok(0) => {}
            Ok(read) => {
                offset += read;
                if expected.is_none() && offset >= 12 {
                    let total = usize::from(u16::from_le_bytes([bytes[10], bytes[11]]));
                    if !(ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES..=MAX_RECEIPT_BYTES)
                        .contains(&total)
                    {
                        return Err("AVR Host reported an invalid terminal receipt length".into());
                    }
                    expected = Some(total);
                }
                if expected == Some(offset) {
                    bytes.truncate(offset);
                    return Ok(bytes);
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
    }
}

fn identity_hex(identity: AssignedIdentity) -> String {
    identity
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_identity_requires_observed_avr_boot_serial() {
        assert!(valid_boot_id("avr-00000001"));
        assert!(valid_boot_id("avr-deadbeef"));
        for invalid in ["avr-1", "AVR-00000001", "avr-0000000g", "pico-00000001"] {
            assert!(!valid_boot_id(invalid));
        }
    }

    #[test]
    fn every_generic_terminal_disposition_has_an_exact_receipt_name() {
        assert_eq!(
            disposition_name(AssignedTerminalDisposition::Completed),
            "completed"
        );
        assert_eq!(
            disposition_name(AssignedTerminalDisposition::Refused),
            "refused"
        );
        assert_eq!(
            disposition_name(AssignedTerminalDisposition::Failed),
            "failed"
        );
        assert_eq!(
            disposition_name(AssignedTerminalDisposition::Cancelled),
            "cancelled"
        );
    }

    #[test]
    fn receive_only_proof_must_be_exact_and_stable_high() {
        let directory =
            std::env::temp_dir().join(format!("conduit-avr-rx-proof-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("proof.json");
        let mut proof = serde_json::json!({
            "schema": "conduit.avr-promicro/create-rx-check@2",
            "outcome": "stable-high",
            "proof_class": "physical-gpio-receive-only",
            "samples": 2048,
            "high_samples": 2048,
            "low_samples": 0,
            "transitions": 0,
            "create_uart": "isolated-no-transmitter",
            "create_stopped": true,
            "attended": true,
            "wheels_clear": true,
            "d1_disconnected_or_high_impedance": true,
            "common_ground_verified": true,
            "rx_voltage_compatible": true
        });
        fs::write(&path, serde_json::to_vec(&proof).unwrap()).unwrap();
        validate_rx_proof(&path).unwrap();
        proof["transitions"] = 1.into();
        fs::write(&path, serde_json::to_vec(&proof).unwrap()).unwrap();
        assert!(validate_rx_proof(&path).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
