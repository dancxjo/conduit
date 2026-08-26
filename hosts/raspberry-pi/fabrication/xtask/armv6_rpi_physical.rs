use std::{
    fs::{self, File},
    io::Read,
    os::unix::fs::FileTypeExt,
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli::GlobalOpts;

use super::{
    armv6_rpi_board::Armv6RpiBoard, profile::Paths, report::git_head, ConduitosArch, ConduitosError,
};

const ENTRY_PREFIX: &str = "CONDUIT_ARMV6_RPI_ENTRY_SIGN ";
const KERNEL_PREFIX: &str = "CONDUIT_KERNEL_SIGN ";
const IDENTITY_PREFIX: &str = "CONDUIT_ARMV6_RPI_A3_IDENTITY ";
const MAX_TRANSCRIPT_BYTES: usize = 8192;

#[derive(Serialize)]
struct PhysicalRecord {
    schema: &'static str,
    proof_class: &'static str,
    base_commit: String,
    architecture: &'static str,
    machine: &'static str,
    board: &'static str,
    firmware_board_revision: String,
    serial_device: String,
    baud: u32,
    timeout_seconds: u64,
    transcript_bytes: usize,
    transcript_sha256: String,
    semantic_result: &'static str,
    physical_boot_proved: bool,
}

pub fn execute(
    board: Armv6RpiBoard,
    serial_device: &Path,
    timeout_seconds: u64,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    inspect_serial(serial_device)?;
    if opts.dry_run {
        println!(
            "capture at most {MAX_TRANSCRIPT_BYTES} bytes for {timeout_seconds}s from {} at 115200 8N1 and require exact {} firmware revision and A3 Signs",
            serial_device.display(),
            board.id()
        );
        return Ok(());
    }
    configure_serial(serial_device)?;
    if !opts.quiet {
        eprintln!(
            "Waiting up to {timeout_seconds}s for a fresh {} boot on {}",
            board.id(),
            serial_device.display()
        );
    }
    let transcript = capture(serial_device, Duration::from_secs(timeout_seconds))?;
    let text = String::from_utf8(transcript.clone())
        .map_err(|error| refusal("physical-uart-invalid-utf8", error))?;
    let entry_line = one_sign(&text, ENTRY_PREFIX, "entry")?;
    let kernel_line = one_sign(&text, KERNEL_PREFIX, "kernel")?;
    let identity_line = one_sign(&text, IDENTITY_PREFIX, "identity")?;
    let entry: serde_json::Value = parse(entry_line, ENTRY_PREFIX)?;
    let kernel: serde_json::Value = parse(kernel_line, KERNEL_PREFIX)?;
    let identity: serde_json::Value = parse(identity_line, IDENTITY_PREFIX)?;
    let revision = entry["firmware_board_revision"].as_str().ok_or_else(|| {
        refusal(
            "physical-board-revision-missing",
            "firmware returned no revision",
        )
    })?;
    let revision_value = u32::from_str_radix(revision, 16)
        .map_err(|error| refusal("physical-board-revision-invalid", error))?;
    if !board.accepts_revision(revision_value) {
        return Err(refusal(
            "physical-board-identity-mismatch",
            format!("{} does not identify {}", revision, board.id()),
        ));
    }
    let paths = Paths::new(ConduitosArch::Armv6)?;
    let commit = git_head(&paths.root)?;
    if entry["board_target"] != board.id()
        || kernel["status"] != "accepted"
        || kernel["arch"] != "armv6"
        || kernel["build_id"] != format!("conduitos-build/{commit}/{}/v1", board.identity_slug())
        || kernel["semantic_result"] != "HELLO, CONDUITOS"
        || kernel["allocation_stable_during_play"] != true
        || kernel["timer_irq_wakes"] != 1
        || kernel["pending_host_operations"] != 0
        || identity["image_id"] != format!("conduitos-image/{commit}/{}/v1", board.identity_slug())
        || identity["a3_ordinary_form_claimed"] != true
    {
        return Err(refusal(
            "physical-a3-sign-invalid",
            "UART Signs do not prove the exact current-head ordinary Form/Plan/Play",
        ));
    }
    let digest = format!("{:x}", Sha256::digest(&transcript));
    let record = PhysicalRecord {
        schema: "conduit.conduitos.armv6-rpi-physical-uart/v1",
        proof_class: "physical-firmware-identified-uart-ordinary-form-plan-play",
        base_commit: commit,
        architecture: "armv6",
        machine: "BCM2835/ARM1176JZF-S",
        board: board.id(),
        firmware_board_revision: revision.to_owned(),
        serial_device: serial_device.display().to_string(),
        baud: 115_200,
        timeout_seconds,
        transcript_bytes: transcript.len(),
        transcript_sha256: digest,
        semantic_result: "HELLO, CONDUITOS",
        physical_boot_proved: true,
    };
    fs::write(
        paths
            .target
            .join(format!("{}-physical-uart.txt", board.artifact_slug())),
        transcript,
    )
    .map_err(|error| refusal("physical-record-write-failed", error))?;
    fs::write(
        paths
            .target
            .join(format!("{}-physical.json", board.artifact_slug())),
        serde_json::to_vec_pretty(&record)
            .map_err(|error| refusal("physical-record-write-failed", error))?,
    )
    .map_err(|error| refusal("physical-record-write-failed", error))?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&record)
                .map_err(|error| refusal("physical-record-write-failed", error))?
        );
    } else if !opts.quiet {
        println!("Accepted physical {} UART proof", board.id());
    }
    Ok(())
}

fn inspect_serial(path: &Path) -> Result<(), ConduitosError> {
    let metadata =
        fs::metadata(path).map_err(|error| refusal("serial-device-unavailable", error))?;
    if !metadata.file_type().is_char_device() {
        return Err(refusal(
            "serial-device-invalid",
            format!("{} is not a character device", path.display()),
        ));
    }
    Ok(())
}

fn configure_serial(path: &Path) -> Result<(), ConduitosError> {
    let status = Command::new("stty")
        .args(["-F"])
        .arg(path)
        .args([
            "115200", "cs8", "-cstopb", "-parenb", "raw", "-echo", "min", "0", "time", "1",
        ])
        .status()
        .map_err(|error| refusal("serial-configuration-failed", error))?;
    if !status.success() {
        return Err(refusal("serial-configuration-failed", status));
    }
    Ok(())
}

fn capture(path: &Path, timeout: Duration) -> Result<Vec<u8>, ConduitosError> {
    let mut file = File::open(path).map_err(|error| refusal("serial-open-failed", error))?;
    let deadline = Instant::now() + timeout;
    let mut transcript = Vec::new();
    let mut buffer = [0_u8; 512];
    while Instant::now() < deadline {
        match file.read(&mut buffer) {
            Ok(0) => thread::sleep(Duration::from_millis(10)),
            Ok(count) => {
                if transcript.len() + count > MAX_TRANSCRIPT_BYTES {
                    return Err(refusal(
                        "physical-uart-transcript-pressure",
                        transcript.len() + count,
                    ));
                }
                transcript.extend_from_slice(&buffer[..count]);
                if transcript
                    .windows(IDENTITY_PREFIX.len())
                    .any(|window| window == IDENTITY_PREFIX.as_bytes())
                    && transcript.ends_with(b"}\n")
                {
                    return Ok(transcript);
                }
            }
            Err(error) => return Err(refusal("serial-read-failed", error)),
        }
    }
    Err(refusal(
        "physical-uart-timeout",
        format!("captured {} bytes", transcript.len()),
    ))
}

fn one_sign<'a>(text: &'a str, prefix: &str, name: &str) -> Result<&'a str, ConduitosError> {
    let lines = text
        .lines()
        .filter(|line| line.starts_with(prefix))
        .collect::<Vec<_>>();
    if lines.len() != 1 {
        return Err(refusal(
            "physical-sign-cardinality-invalid",
            format!("expected one {name} Sign, found {}", lines.len()),
        ));
    }
    Ok(lines[0])
}

fn parse(line: &str, prefix: &str) -> Result<serde_json::Value, ConduitosError> {
    serde_json::from_str(&line[prefix.len()..])
        .map_err(|error| refusal("physical-sign-invalid", error))
}

fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_files_and_wrong_revision_refuse() {
        assert!(inspect_serial(Path::new("Cargo.toml")).is_err());
        assert!(Armv6RpiBoard::BPlusV1_2.accepts_revision(0x900032));
        assert!(!Armv6RpiBoard::BPlusV1_2.accepts_revision(0x900093));
        assert!(Armv6RpiBoard::ZeroV1.accepts_revision(0x900093));
    }
}
