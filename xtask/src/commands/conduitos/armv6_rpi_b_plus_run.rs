use std::{
    fs,
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    armv6_rpi_b_plus_a0,
    profile::Paths,
    report::{git_head, sha256_file},
    ConduitosArch, ConduitosError,
};

const MACHINE: &str = "raspi0";
const SIGN: &str = "CONDUIT_ARMV6_RPI_ENTRY_SIGN {\"schema\":\"conduit.conduitos.armv6-rpi-entry/v1\",\"status\":\"entered\",\"architecture\":\"armv6\",\"machine\":\"BCM2835/ARM1176JZF-S\",\"board_target\":\"raspberry-pi-model-b-plus-v1.2\",\"boot_mechanism\":\"direct-kernel\",\"runtime_bases_available\":false}";
const MAXIMUM_TRANSCRIPT_BYTES: usize = 4096;

#[derive(Serialize)]
struct RunRecord {
    schema: &'static str,
    proof_class: &'static str,
    base_commit: String,
    architecture: &'static str,
    machine_target: &'static str,
    emulator_machine: &'static str,
    qemu_version: String,
    kernel_image_sha256: String,
    load_address: u32,
    sign: &'static str,
    transcript_bytes: usize,
    runtime_bases_available: bool,
    physical_boot_claimed: bool,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        armv6_rpi_b_plus_a0::execute(opts)?;
        println!(
            "qemu-system-arm -M {MACHINE} -device loader,file=kernel.img,addr=0x8000,cpu-num=0,force-raw=on -serial stdio -display none -no-reboot"
        );
        return Ok(());
    }
    armv6_rpi_b_plus_a0::execute(opts)?;
    let paths = Paths::new(ConduitosArch::Armv6)?;
    let kernel = paths.target.join("kernel.img");
    let qemu_version = command_text(
        "qemu-system-arm",
        &["--version"],
        "armv6-emulator-unavailable",
    )?;
    let mut child = Command::new("qemu-system-arm")
        .args(["-M", MACHINE, "-device"])
        .arg(format!(
            "loader,file={},addr=0x8000,cpu-num=0,force-raw=on",
            kernel.display()
        ))
        .args([
            "-serial",
            "stdio",
            "-monitor",
            "none",
            "-display",
            "none",
            "-no-reboot",
        ])
        .current_dir(&paths.root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| refusal("armv6-emulator-unavailable", error))?;
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if child
            .try_wait()
            .map_err(|error| refusal("armv6-emulator-failed", error))?
            .is_some()
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    if child
        .try_wait()
        .map_err(|error| refusal("armv6-emulator-failed", error))?
        .is_none()
    {
        child
            .kill()
            .map_err(|error| refusal("armv6-emulator-failed", error))?;
    }
    let mut transcript = Vec::new();
    child
        .stdout
        .take()
        .ok_or_else(|| refusal("armv6-emulator-failed", "stdout pipe missing"))?
        .take(MAXIMUM_TRANSCRIPT_BYTES as u64 + 1)
        .read_to_end(&mut transcript)
        .map_err(|error| refusal("armv6-emulator-failed", error))?;
    let output = child
        .wait_with_output()
        .map_err(|error| refusal("armv6-emulator-failed", error))?;
    transcript.extend_from_slice(&output.stderr);
    if transcript.len() > MAXIMUM_TRANSCRIPT_BYTES {
        return Err(refusal(
            "armv6-emulator-transcript-pressure",
            transcript.len(),
        ));
    }
    let transcript_text = String::from_utf8(transcript)
        .map_err(|error| refusal("armv6-emulator-transcript-invalid", error))?;
    if transcript_text.lines().filter(|line| *line == SIGN).count() != 1 {
        return Err(refusal("armv6-entry-sign-missing", transcript_text.trim()));
    }
    let record = RunRecord {
        schema: "conduit.conduitos.armv6-rpi-b-plus-emulator-run/v1",
        proof_class: "freestanding-emulator-entry-only",
        base_commit: git_head(&paths.root)?,
        architecture: "armv6",
        machine_target: "BCM2835/ARM1176JZF-S Raspberry Pi Model B+ v1.2",
        emulator_machine: MACHINE,
        qemu_version: qemu_version.lines().next().unwrap_or_default().to_owned(),
        kernel_image_sha256: sha256_file(&kernel)?,
        load_address: 0x8000,
        sign: SIGN,
        transcript_bytes: transcript_text.len(),
        runtime_bases_available: false,
        physical_boot_claimed: false,
    };
    fs::write(
        paths.target.join("emulator-run.json"),
        serde_json::to_vec_pretty(&record).map_err(|error| refusal("run-record-failed", error))?,
    )
    .map_err(|error| refusal("run-record-failed", error))?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&record)
                .map_err(|error| refusal("run-record-failed", error))?
        );
    } else if !opts.quiet {
        println!("{SIGN}");
    }
    Ok(())
}

fn command_text(
    program: &str,
    args: &[&str],
    reason: &'static str,
) -> Result<String, ConduitosError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| refusal(reason, error))?;
    if !output.status.success() {
        return Err(refusal(reason, output.status));
    }
    String::from_utf8(output.stdout).map_err(|error| refusal(reason, error))
}

fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}
