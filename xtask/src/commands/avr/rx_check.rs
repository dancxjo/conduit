use std::{
    fs::{File, OpenOptions},
    io::{ErrorKind, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use clap::Args;
use serde::Serialize;

use super::{
    config_path, provision, require_success, resolve_upload_port, run_build_receive_only,
    verify_device, write_receipt, PhysicalGate, EXPECTED_BY_ID, FQBN,
};
use crate::{cli::GlobalOpts, workspace::workspace_root};

const REQUEST: &[u8; 8] = b"RXDIAG01";
const RECEIPT_BYTES: usize = 28;
const SAMPLE_COUNT: u16 = 2_048;

#[derive(Args, Debug)]
pub(super) struct RxCheckArgs {
    #[arg(long)]
    port: PathBuf,
    #[arg(long)]
    artifact_sha256: String,
    #[arg(long)]
    create_stopped: bool,
    #[arg(long)]
    attended: bool,
    #[arg(long)]
    wheels_clear: bool,
    #[arg(long)]
    d1_disconnected_or_high_impedance: bool,
    #[arg(long)]
    common_ground_verified: bool,
    #[arg(long)]
    rx_voltage_compatible: bool,
    #[arg(long, default_value = "target/avr-promicro/rx-check-receipt.json")]
    receipt: PathBuf,
}

#[derive(Debug, Serialize)]
struct RxCheckReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    source_sha: String,
    source_digest_sha256: String,
    build_id: String,
    artifact_sha256: String,
    port: String,
    samples: u16,
    high_samples: u16,
    low_samples: u16,
    transitions: u16,
    duration_us: u32,
    create_stopped: bool,
    attended: bool,
    wheels_clear: bool,
    d1_disconnected_or_high_impedance: bool,
    common_ground_verified: bool,
    rx_voltage_compatible: bool,
    create_uart: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Evidence {
    high: u16,
    low: u16,
    transitions: u16,
    duration_us: u32,
}

pub(super) fn run(args: RxCheckArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    validate_request(&args)?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would rebuild and verify receive-only artifact {}, verify {}, flash it, reopen the exact CDC device once, and sample Create RX {SAMPLE_COUNT} times with D1 input and USART1 disabled",
                args.artifact_sha256,
                args.port.display()
            );
        }
        return Ok(());
    }

    let built = run_build_receive_only(
        Path::new("target/avr-promicro/receive-only-build-receipt.json"),
        opts,
    )?;
    if built.artifact_sha256 != args.artifact_sha256 {
        return Err(format!(
            "AVR receive-only artifact digest mismatch: expected {}, built {}",
            args.artifact_sha256, built.artifact_sha256
        )
        .into());
    }
    verify_device(&args.port)?;
    let upload_port = resolve_upload_port(&args.port)?;
    let root = workspace_root()?;
    let cli = provision(&root)?;
    let output = Command::new(cli)
        .args(["upload", "--fqbn", FQBN, "--port"])
        .arg(&upload_port)
        .args(["--input-file"])
        .arg(&built.path)
        .args(["--config-file"])
        .arg(config_path(&root))
        .output()?;
    require_success(&output, "guarded receive-only AVR flash")?;
    wait_for_exact_device(&args.port, Duration::from_secs(15))?;
    configure_serial(&args.port)?;
    let mut device = open_nonblocking(&args.port)?;
    write_bounded(&mut device, REQUEST, Duration::from_secs(2))?;
    let mut response = [0; RECEIPT_BYTES];
    read_bounded(&mut device, &mut response, Duration::from_secs(3))?;
    let evidence = parse_receipt(&response)?;
    let outcome = classify(evidence);
    let record = RxCheckReceipt {
        schema: "conduit.avr-promicro/create-rx-check@2",
        outcome,
        proof_class: "physical-gpio-receive-only",
        source_sha: built.identity.source_sha,
        source_digest_sha256: built.identity.source_digest_sha256,
        build_id: built.identity.build_id,
        artifact_sha256: built.artifact_sha256,
        port: args.port.display().to_string(),
        samples: SAMPLE_COUNT,
        high_samples: evidence.high,
        low_samples: evidence.low,
        transitions: evidence.transitions,
        duration_us: evidence.duration_us,
        create_stopped: args.create_stopped,
        attended: args.attended,
        wheels_clear: args.wheels_clear,
        d1_disconnected_or_high_impedance: args.d1_disconnected_or_high_impedance,
        common_ground_verified: args.common_ground_verified,
        rx_voltage_compatible: args.rx_voltage_compatible,
        create_uart: "isolated-no-transmitter",
    };
    write_receipt(&root.join(args.receipt), &record, opts)?;
    if outcome != "stable-high" {
        return Err(format!(
            "Create RX boundary was not stable high: high={} low={} transitions={}",
            evidence.high, evidence.low, evidence.transitions
        )
        .into());
    }
    Ok(())
}

fn classify(evidence: Evidence) -> &'static str {
    if evidence.high == SAMPLE_COUNT && evidence.low == 0 && evidence.transitions == 0 {
        "stable-high"
    } else {
        "not-stable-high"
    }
}

fn validate_request(args: &RxCheckArgs) -> Result<(), Box<dyn std::error::Error>> {
    PhysicalGate {
        create_stopped: args.create_stopped,
        attended: args.attended,
        wheels_clear: args.wheels_clear,
    }
    .validate("receive-only Create RX check")?;
    if !args.d1_disconnected_or_high_impedance
        || !args.common_ground_verified
        || !args.rx_voltage_compatible
    {
        return Err("AVR receive-only Create RX check requires --d1-disconnected-or-high-impedance --common-ground-verified --rx-voltage-compatible".into());
    }
    if args.port.file_name().and_then(|name| name.to_str()) != Some(EXPECTED_BY_ID)
        || args.port.parent() != Some(Path::new("/dev/serial/by-id"))
    {
        return Err(format!(
            "AVR receive-only Create RX check requires exact path /dev/serial/by-id/{EXPECTED_BY_ID}"
        )
        .into());
    }
    if args.artifact_sha256.len() != 64
        || !args
            .artifact_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("AVR receive-only Create RX check requires one exact SHA-256 digest".into());
    }
    Ok(())
}

fn parse_receipt(bytes: &[u8]) -> Result<Evidence, Box<dyn std::error::Error>> {
    if bytes.len() != RECEIPT_BYTES
        || &bytes[..8] != b"CNDRX001"
        || u16_at(bytes, 8)? != 1
        || usize::from(u16_at(bytes, 10)?) != bytes.len()
        || u16_at(bytes, 12)? != SAMPLE_COUNT
        || u16_at(bytes, 14)?.checked_add(u16_at(bytes, 16)?) != Some(SAMPLE_COUNT)
        || bytes[24] != 0
        || bytes[25] != 0
        || u16_at(bytes, 26)? != 0
    {
        return Err("invalid receive-only diagnostic receipt".into());
    }
    Ok(Evidence {
        high: u16_at(bytes, 14)?,
        low: u16_at(bytes, 16)?,
        transitions: u16_at(bytes, 18)?,
        duration_us: u32_at(bytes, 20)?,
    })
}

fn configure_serial(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("stty")
        .args(["-F"])
        .arg(path)
        .args(["raw", "-echo", "-hupcl", "115200"])
        .output()?;
    require_success(&output, "receive-only CDC configuration")
}

fn open_nonblocking(path: &Path) -> Result<File, Box<dyn std::error::Error>> {
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(0o00000400)
        .open(path)?)
}

fn write_bounded(
    device: &mut File,
    bytes: &[u8],
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    let mut offset = 0;
    while offset < bytes.len() && Instant::now() < deadline {
        match device.write(&bytes[offset..]) {
            Ok(0) => thread::sleep(Duration::from_millis(5)),
            Ok(written) => offset += written,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error.into()),
        }
    }
    if offset != bytes.len() {
        return Err("timed out writing receive-only diagnostic request".into());
    }
    Ok(())
}

fn read_bounded(
    device: &mut File,
    bytes: &mut [u8],
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    let mut offset = 0;
    while offset < bytes.len() && Instant::now() < deadline {
        match device.read(&mut bytes[offset..]) {
            Ok(0) => thread::sleep(Duration::from_millis(5)),
            Ok(read) => offset += read,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error.into()),
        }
    }
    if offset != bytes.len() {
        return Err("timed out reading receive-only diagnostic receipt".into());
    }
    Ok(())
}

fn wait_for_exact_device(path: &Path, timeout: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() && verify_device(path).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("receive-only image did not re-enumerate at the exact CDC identity".into())
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, Box<dyn std::error::Error>> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or("truncated receive-only receipt")?
            .try_into()?,
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, Box<dyn std::error::Error>> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or("truncated receive-only receipt")?
            .try_into()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_receipt_parses_and_isolation_or_accounting_drift_refuses() {
        let mut bytes = [0; RECEIPT_BYTES];
        bytes[..8].copy_from_slice(b"CNDRX001");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&(RECEIPT_BYTES as u16).to_le_bytes());
        bytes[12..14].copy_from_slice(&SAMPLE_COUNT.to_le_bytes());
        bytes[14..16].copy_from_slice(&SAMPLE_COUNT.to_le_bytes());
        bytes[20..24].copy_from_slice(&2_048_u32.to_le_bytes());
        let stable = Evidence {
            high: SAMPLE_COUNT,
            low: 0,
            transitions: 0,
            duration_us: 2_048,
        };
        assert_eq!(parse_receipt(&bytes).unwrap(), stable);
        assert_eq!(classify(stable), "stable-high");
        assert_eq!(
            classify(Evidence {
                high: 0,
                low: SAMPLE_COUNT,
                transitions: 0,
                duration_us: 2_048,
            }),
            "not-stable-high"
        );
        assert_eq!(
            classify(Evidence {
                high: SAMPLE_COUNT - 1,
                low: 1,
                transitions: 2,
                duration_us: 2_048,
            }),
            "not-stable-high"
        );

        bytes[25] = 1;
        assert!(parse_receipt(&bytes).is_err());
        bytes[25] = 0;
        bytes[16] = 1;
        assert!(parse_receipt(&bytes).is_err());
    }
}
