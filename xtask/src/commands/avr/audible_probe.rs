//! One bounded motion-free audible TX then telemetry RX hardware diagnostic.

use std::{
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use clap::Args;
use serde::Serialize;

use super::{
    build_identity::{digest_compiled_sources, EmbeddedBuildIdentity, BUILD_ID_SCHEMA},
    observe::{configure_serial, open_nonblocking, validate_rx_proof, wait_for_runtime},
    relative,
    rust_firmware::{self, AVR_HAL_REVISION, FIRMWARE, RUST_TOOLCHAIN},
    sha256_file, upload_artifact, validate_sizes, write_receipt, BuildReceipt, BuiltArtifact,
    PhysicalGate, ARDUINO_AVR_VERSION, CLI_VERSION, FQBN, MAX_FLASH_BYTES, MAX_SRAM_BYTES,
    MAX_STATIC_SRAM_BYTES, SPARKFUN_AVR_VERSION, STACK_RESERVE_BYTES,
};
use crate::{cli::GlobalOpts, workspace::workspace_root};

const REPORT_MAGIC: u8 = 0xa5;
const BOOTLOADER_WAIT: Duration = Duration::from_secs(300);

#[derive(Args, Debug)]
pub(super) struct AudibleProbeArgs {
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
    #[arg(long, default_value = "target/avr-promicro/audible-probe-receipt.json")]
    receipt: PathBuf,
}

#[derive(Serialize)]
struct AudibleProbeReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    artifact_sha256: String,
    boot_id: String,
    stage: u8,
    result: u8,
    observed_oi_mode: Option<u8>,
    contact_body_sectors: Option<u8>,
    uart_baud: u32,
    transmitted_program: &'static str,
    motion_opcode_admitted: bool,
    create_stopped: bool,
    attended: bool,
    wheels_clear: bool,
}

pub(super) fn run(
    args: AudibleProbeArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    PhysicalGate {
        create_stopped: args.create_stopped,
        attended: args.attended,
        wheels_clear: args.wheels_clear,
    }
    .validate("audible probe")?;
    if !args.common_ground_verified || !args.rx_voltage_compatible {
        return Err("AVR audible probe requires common ground and compatible RX voltage".into());
    }
    let root = workspace_root()?;
    validate_rx_proof(&root.join(&args.rx_proof))?;
    if opts.dry_run {
        return Ok(());
    }
    let built = build(
        Path::new("target/avr-promicro/audible-probe-build-receipt.json"),
        opts,
    )?;
    let upload_port = super::rx_check::wait_for_bootloader_port(BOOTLOADER_WAIT)?;
    upload_artifact(
        &root,
        &upload_port,
        &built.path,
        "bounded motion-free AVR audible probe flash",
    )?;
    let runtime = wait_for_runtime(Duration::from_secs(15))?;
    configure_serial(&runtime.path)?;
    let mut device = open_nonblocking(&runtime.path)?;
    write_trigger(&mut device, Duration::from_secs(2))?;
    let report = read_report(&mut device, Duration::from_secs(5))?;
    let completed = report[1] == 0 && report[2] == 3;
    let receipt = AudibleProbeReceipt {
        schema: "conduit.avr-promicro/audible-uart-probe@1",
        outcome: if completed { "completed" } else { "failed" },
        proof_class: "physical-motion-free-create-uart-hil",
        artifact_sha256: built.artifact_sha256,
        boot_id: runtime.boot_id,
        stage: report[1],
        result: report[2],
        observed_oi_mode: completed.then_some(report[2]),
        contact_body_sectors: completed.then_some(report[3]),
        uart_baud: 19_200,
        transmitted_program:
            "shared-create1-start-full-define-song-play-song-query-mode-query-group-zero",
        motion_opcode_admitted: false,
        create_stopped: args.create_stopped,
        attended: args.attended,
        wheels_clear: args.wheels_clear,
    };
    let receipt_path = root.join(args.receipt);
    write_receipt(&receipt_path, &receipt, opts)?;
    if !completed {
        return Err(format!(
            "AVR audible probe failed at stage={} result={}; wrote {}",
            report[1],
            report[2],
            receipt_path.display()
        )
        .into());
    }
    Ok(())
}

fn write_trigger(
    device: &mut std::fs::File,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    loop {
        if !super::rx_check::wait_ready(device, libc::POLLOUT, deadline)? {
            return Err("timed out arming AVR audible probe report".into());
        }
        match device.write(&[0x51]) {
            Ok(1) => return Ok(()),
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
    }
}

pub(super) fn build(
    receipt: &Path,
    opts: &GlobalOpts,
) -> Result<BuiltArtifact, Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let artifact = root
        .join("target/avr-promicro/build")
        .join("conduit-avr-audible-probe.hex");
    let identity = EmbeddedBuildIdentity::new(
        super::git_head(&root)?,
        digest_compiled_sources(&root.join(FIRMWARE))?,
        "audible-uart-hil",
    );
    if opts.dry_run {
        return Ok(BuiltArtifact {
            path: artifact,
            artifact_sha256: String::new(),
            identity,
        });
    }
    let built = rust_firmware::build_audible_probe(&root)?;
    if built.hex != artifact {
        return Err("Rust AVR audible probe returned an unexpected artifact path".into());
    }
    validate_sizes(built.flash_bytes, built.sram_bytes)?;
    let digest = sha256_file(&artifact)?;
    let record = BuildReceipt {
        schema: "conduit.avr-promicro/build@4",
        outcome: "built",
        proof_class: "machine-only-contract-compile",
        profile: "audible-uart-hil",
        build_id_schema: BUILD_ID_SCHEMA,
        build_id: identity.build_id.clone(),
        source_sha: identity.source_sha.clone(),
        source_digest_sha256: identity.source_digest_sha256.clone(),
        target: FQBN,
        board_variant: "atmega32u4-5v-16mhz-usb-pid-9206",
        arduino_cli: CLI_VERSION,
        arduino_avr: ARDUINO_AVR_VERSION,
        sparkfun_avr: SPARKFUN_AVR_VERSION,
        rust_toolchain: RUST_TOOLCHAIN,
        avr_hal_revision: AVR_HAL_REVISION,
        artifact: relative(&root, &artifact)?,
        artifact_sha256: digest.clone(),
        flash_bytes: built.flash_bytes,
        flash_limit: MAX_FLASH_BYTES,
        sram_bytes: built.sram_bytes,
        sram_limit: MAX_SRAM_BYTES,
        static_sram_limit: MAX_STATIC_SRAM_BYTES,
        stack_reserve_bytes: STACK_RESERVE_BYTES,
        create_uart: "bounded-motion-free-audible-tx-then-telemetry-rx-19200-8n1",
    };
    write_receipt(&root.join(receipt), &record, opts)?;
    Ok(BuiltArtifact {
        path: artifact,
        artifact_sha256: digest,
        identity,
    })
}

fn read_report(
    device: &mut std::fs::File,
    timeout: Duration,
) -> Result<[u8; 4], Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    let mut report = [0_u8; 4];
    let mut offset = 0;
    while offset < report.len() {
        if !super::rx_check::wait_ready(device, libc::POLLIN, deadline)? {
            return Err("timed out reading AVR audible probe report".into());
        }
        match device.read(&mut report[offset..]) {
            Ok(0) => {}
            Ok(read) => offset += read,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
    }
    if report[0] != REPORT_MAGIC {
        return Err("AVR audible probe reported invalid framing".into());
    }
    Ok(report)
}
