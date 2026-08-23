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
const ENTRY_SIGN: &str = "CONDUIT_ARMV6_RPI_ENTRY_SIGN {\"schema\":\"conduit.conduitos.armv6-rpi-entry/v1\",\"status\":\"entered\",\"architecture\":\"armv6\",\"machine\":\"BCM2835/ARM1176JZF-S\",\"board_target\":\"raspberry-pi-model-b-plus-v1.2\",\"boot_mechanism\":\"direct-kernel\",\"runtime_bases_available\":true}";
const KERNEL_SIGN_PREFIX: &str = "CONDUIT_KERNEL_SIGN ";
const IDENTITY_SIGN_PREFIX: &str = "CONDUIT_ARMV6_RPI_A3_IDENTITY ";
const MAXIMUM_TRANSCRIPT_BYTES: usize = 8192;

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
    entry_sign: &'static str,
    kernel_sign: String,
    identity_sign: String,
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
    let kernel_path = paths.target.join("kernel.img");
    let qemu_version = command_text(
        "qemu-system-arm",
        &["--version"],
        "armv6-emulator-unavailable",
    )?;
    let mut child = Command::new("qemu-system-arm")
        .args(["-M", MACHINE, "-device"])
        .arg(format!(
            "loader,file={},addr=0x8000,cpu-num=0,force-raw=on",
            kernel_path.display()
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
    let deadline = Instant::now() + Duration::from_secs(10);
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
    if transcript_text
        .lines()
        .filter(|line| *line == ENTRY_SIGN)
        .count()
        != 1
    {
        return Err(refusal("armv6-entry-sign-missing", transcript_text.trim()));
    }
    let kernel_sign = transcript_text
        .lines()
        .find(|line| line.starts_with(KERNEL_SIGN_PREFIX))
        .ok_or_else(|| refusal("armv6-kernel-sign-missing", transcript_text.trim()))?;
    let kernel: serde_json::Value = serde_json::from_str(&kernel_sign[KERNEL_SIGN_PREFIX.len()..])
        .map_err(|error| refusal("armv6-kernel-sign-invalid", error))?;
    let identity_sign = transcript_text
        .lines()
        .find(|line| line.starts_with(IDENTITY_SIGN_PREFIX))
        .ok_or_else(|| refusal("armv6-identity-sign-missing", transcript_text.trim()))?;
    let identity: serde_json::Value =
        serde_json::from_str(&identity_sign[IDENTITY_SIGN_PREFIX.len()..])
            .map_err(|error| refusal("armv6-identity-sign-invalid", error))?;
    let commit = git_head(&paths.root)?;
    if kernel["schema"] != "conduit.conduitos.kernel-sign/v2"
        || kernel["status"] != "accepted"
        || kernel["arch"] != "armv6"
        || kernel["build_id"] != format!("conduitos-build/{commit}/armv6-rpi-b-plus/v1")
        || kernel["pipeline"] != "check-plan-lower-kernel"
        || kernel["semantic_result"] != "HELLO, CONDUITOS"
        || kernel["allocation_stable_during_play"] != true
        || kernel["timer_irq_wakes"] != 1
        || kernel["pending_host_operations"] != 0
        || identity["image_id"] != format!("conduitos-image/{commit}/armv6-rpi-b-plus/v1")
        || identity["wake_source"] != "bcm2835-system-timer-compare-1"
        || identity["wake_irq"] != 1
        || identity["a3_ordinary_form_claimed"] != true
    {
        return Err(refusal("armv6-a3-sign-invalid", kernel));
    }
    let record = RunRecord {
        schema: "conduit.conduitos.armv6-rpi-b-plus-emulator-run/v1",
        proof_class: "freestanding-emulator-ordinary-form-plan-play",
        base_commit: commit,
        architecture: "armv6",
        machine_target: "BCM2835/ARM1176JZF-S Raspberry Pi Model B+ v1.2",
        emulator_machine: MACHINE,
        qemu_version: qemu_version.lines().next().unwrap_or_default().to_owned(),
        kernel_image_sha256: sha256_file(&kernel_path)?,
        load_address: 0x8000,
        entry_sign: ENTRY_SIGN,
        kernel_sign: kernel_sign.to_owned(),
        identity_sign: identity_sign.to_owned(),
        transcript_bytes: transcript_text.len(),
        runtime_bases_available: true,
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
        println!("{ENTRY_SIGN}");
        println!("{kernel_sign}");
        println!("{identity_sign}");
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
