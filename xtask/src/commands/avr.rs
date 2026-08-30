use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use clap::{Args, Subcommand};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{cli::GlobalOpts, workspace::workspace_root};

mod audible_probe;
mod avr_toolchain;
mod build_identity;
mod release;
mod observe;
mod plan;
mod rust_firmware;
mod rx_check;

use avr_toolchain::{
    avrdude_bin, avrdude_config, ARDUINO_AVR_VERSION, CLI_VERSION, SPARKFUN_AVR_VERSION,
};
use build_identity::{digest_compiled_sources, EmbeddedBuildIdentity, BUILD_ID_SCHEMA};
use conduit_host_avr_fabrication::{FQBN, SPORE_REGION_START, SRAM_BYTES};
use rust_firmware::{AVR_HAL_REVISION, FIRMWARE, RUST_TOOLCHAIN};
const EXPECTED_BY_ID: &str = "usb-SparkFun_SparkFun_Pro_Micro-if00";
const EXPECTED_VID: &str = "1b4f";
const EXPECTED_PID: &str = "9206";
const MAX_FLASH_BYTES: u64 = SPORE_REGION_START;
const MAX_SRAM_BYTES: u64 = SRAM_BYTES;
const STACK_RESERVE_BYTES: u64 = 1_100;
const MAX_STATIC_SRAM_BYTES: u64 = MAX_SRAM_BYTES - STACK_RESERVE_BYTES;
const AVRDUDE_DEADLINE: Duration = Duration::from_secs(15);

#[derive(Args, Debug)]
pub struct AvrArgs {
    #[command(subcommand)]
    command: AvrCommand,
}

#[derive(Subcommand, Debug)]
enum AvrCommand {
    /// Provision and verify the pinned AVR build boundary without hardware access.
    Check,
    /// Build the exact assigned-Plan Pro Micro Host image and write a receipt.
    Build {
        #[arg(long, default_value = "target/avr-promicro/build-receipt.json")]
        receipt: PathBuf,
    },
    /// Build and seal the exact generic Pro Micro Intel HEX release.
    Release {
        #[arg(long, default_value = "target/creche-avr-release")]
        output: PathBuf,
    },
    /// Build the exact receive-only diagnostic image and write a receipt.
    BuildReceiveOnly {
        #[arg(
            long,
            default_value = "target/avr-promicro/receive-only-build-receipt.json"
        )]
        receipt: PathBuf,
    },
    /// Build the bounded motion-free audible UART diagnostic image.
    BuildAudibleProbe {
        #[arg(
            long,
            default_value = "target/avr-promicro/audible-probe-build-receipt.json"
        )]
        receipt: PathBuf,
    },
    /// Flash and execute one attended receive-only Create RX diagnostic.
    ReceiveOnly(rx_check::RxCheckArgs),
    /// Plan and execute one attended Create contact observation on the AVR Host.
    ObserveContact(observe::ObserveContactArgs),
    /// Run one attended motion-free audible TX then telemetry RX diagnostic.
    AudibleProbe(audible_probe::AudibleProbeArgs),
    /// Flash only after exact artifact, device, and physical gates are supplied.
    Flash {
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
        #[arg(long, default_value = "target/avr-promicro/flash-receipt.json")]
        receipt: PathBuf,
    },
}

#[derive(Debug, Serialize)]
struct BuildReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    profile: &'static str,
    build_id_schema: &'static str,
    build_id: String,
    source_sha: String,
    source_digest_sha256: String,
    target: &'static str,
    board_variant: &'static str,
    arduino_cli: &'static str,
    arduino_avr: &'static str,
    sparkfun_avr: &'static str,
    rust_toolchain: &'static str,
    avr_hal_revision: &'static str,
    artifact: String,
    artifact_sha256: String,
    flash_bytes: u64,
    flash_limit: u64,
    sram_bytes: u64,
    sram_limit: u64,
    static_sram_limit: u64,
    stack_reserve_bytes: u64,
    create_uart: &'static str,
}

struct BuiltArtifact {
    pub(super) path: PathBuf,
    pub(super) artifact_sha256: String,
    pub(super) flash_bytes: u64,
    pub(super) identity: EmbeddedBuildIdentity,
}

pub fn run(args: AvrArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        AvrCommand::Check => run_check(opts),
        AvrCommand::Build { receipt } => run_build(&receipt, opts).map(|_| ()),
        AvrCommand::Release { output } => release::run(&output, opts),
        AvrCommand::BuildReceiveOnly { receipt } => {
            run_build_receive_only(&receipt, opts).map(|_| ())
        }
        AvrCommand::BuildAudibleProbe { receipt } => {
            audible_probe::build(&receipt, opts).map(|_| ())
        }
        AvrCommand::ReceiveOnly(args) => rx_check::run(args, opts),
        AvrCommand::ObserveContact(args) => observe::run(args, opts),
        AvrCommand::AudibleProbe(args) => audible_probe::run(args, opts),
        AvrCommand::Flash {
            port,
            artifact_sha256,
            create_stopped,
            attended,
            wheels_clear,
            receipt,
        } => run_flash(
            &port,
            &artifact_sha256,
            PhysicalGate {
                create_stopped,
                attended,
                wheels_clear,
            },
            &receipt,
            opts,
        ),
    }
}

fn run_build_receive_only(
    receipt: &Path,
    opts: &GlobalOpts,
) -> Result<BuiltArtifact, Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let artifact = root
        .join("target/avr-promicro/build")
        .join("conduit-avr-receive-only.hex");
    let identity = EmbeddedBuildIdentity::new(
        git_head(&root)?,
        digest_compiled_sources(&root.join(FIRMWARE))?,
        "receive-only",
    );
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would build {FQBN} profile=receive-only from {}",
                root.join(FIRMWARE).display()
            );
        }
        return Ok(BuiltArtifact {
            path: artifact,
            artifact_sha256: String::new(),
            identity,
        });
    }
    let built = rust_firmware::build_receive_only(&root)?;
    if built.hex != artifact {
        return Err("Rust AVR receive-only build returned an unexpected artifact path".into());
    }
    validate_sizes(built.flash_bytes, built.sram_bytes)?;
    let digest = sha256_file(&artifact)?;
    let record = BuildReceipt {
        schema: "conduit.avr-promicro/build@4",
        outcome: "built",
        proof_class: "machine-only-contract-compile",
        profile: "receive-only",
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
        create_uart: "isolated-no-transmitter",
    };
    write_receipt(&root.join(receipt), &record, opts)?;
    Ok(BuiltArtifact {
        path: artifact,
        artifact_sha256: digest,
        identity,
    })
}

fn run_check(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    if opts.dry_run {
        if !opts.quiet {
            println!("would provision and verify Rust {RUST_TOOLCHAIN}, AVR HAL {AVR_HAL_REVISION}, Arduino CLI {CLI_VERSION}, and the pinned AVR GCC/upload tools");
        }
        return Ok(());
    }
    rust_firmware::check(&root)?;
    if !opts.quiet {
        println!("AVR boundary ready: {FQBN}");
    }
    Ok(())
}

fn run_build(
    receipt: &Path,
    opts: &GlobalOpts,
) -> Result<BuiltArtifact, Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let artifact = root
        .join("target/avr-promicro/build")
        .join("conduit-avr-promicro-host.hex");
    let identity = EmbeddedBuildIdentity::new(
        git_head(&root)?,
        digest_compiled_sources(&root.join(FIRMWARE))?,
        "assigned-create-host",
    );
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would build {FQBN} profile=assigned-create-host from {}",
                root.join(FIRMWARE).display()
            );
        }
        return Ok(BuiltArtifact {
            path: artifact,
            artifact_sha256: String::new(),
            flash_bytes: 0,
            identity,
        });
    }
    let built = rust_firmware::build(&root)?;
    if built.hex != artifact {
        return Err("Rust AVR build returned an unexpected artifact path".into());
    }
    let flash_bytes = built.flash_bytes;
    let sram_bytes = built.sram_bytes;
    validate_sizes(flash_bytes, sram_bytes)?;
    let digest = sha256_file(&artifact)?;
    let record = BuildReceipt {
        schema: "conduit.avr-promicro/build@4",
        outcome: "built",
        proof_class: "machine-only-contract-compile",
        profile: "assigned-create-host",
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
        flash_bytes,
        flash_limit: MAX_FLASH_BYTES,
        sram_bytes,
        sram_limit: MAX_SRAM_BYTES,
        static_sram_limit: MAX_STATIC_SRAM_BYTES,
        stack_reserve_bytes: STACK_RESERVE_BYTES,
        create_uart: "rust-shared-provider-assigned-plan-dispatch-only",
    };
    write_receipt(&root.join(receipt), &record, opts)?;
    Ok(BuiltArtifact {
        path: artifact,
        artifact_sha256: digest,
        flash_bytes,
        identity,
    })
}

#[derive(Clone, Copy)]
struct PhysicalGate {
    create_stopped: bool,
    attended: bool,
    wheels_clear: bool,
}

impl PhysicalGate {
    fn validate(self, action: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !self.create_stopped || !self.attended || !self.wheels_clear {
            return Err(format!(
                "AVR {action} requires --create-stopped --attended --wheels-clear"
            )
            .into());
        }
        Ok(())
    }
}

fn run_flash(
    port: &Path,
    expected_digest: &str,
    gate: PhysicalGate,
    _receipt: &Path,
    _opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_flash_request(port, expected_digest, gate)?;
    Err("transmit-capable AVR flash remains disabled until the accepted #1965 receive-only receipt is supplied to the deployment entrance".into())
}

fn validate_flash_request(
    port: &Path,
    digest: &str,
    gate: PhysicalGate,
) -> Result<(), Box<dyn std::error::Error>> {
    gate.validate("flash")?;
    if port.file_name().and_then(|name| name.to_str()) != Some(EXPECTED_BY_ID)
        || port.parent() != Some(Path::new("/dev/serial/by-id"))
    {
        return Err(
            format!("AVR flash requires exact path /dev/serial/by-id/{EXPECTED_BY_ID}").into(),
        );
    }
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("AVR flash requires one exact SHA-256 artifact digest".into());
    }
    Ok(())
}

fn verify_device(port: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("udevadm")
        .args(["info", "--query=property", "--name"])
        .arg(port)
        .output()?;
    require_success(&output, "Pro Micro descriptor verification")?;
    let properties = String::from_utf8(output.stdout)?;
    for expected in [
        format!("ID_VENDOR_ID={EXPECTED_VID}"),
        format!("ID_MODEL_ID={EXPECTED_PID}"),
        "ID_SERIAL=SparkFun_SparkFun_Pro_Micro".to_owned(),
    ] {
        if !properties.lines().any(|line| line == expected) {
            return Err(format!("Pro Micro descriptor mismatch: missing {expected}").into());
        }
    }
    Ok(())
}

fn metric(report: &str, prefix: &str, suffix: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let start = report
        .find(prefix)
        .ok_or_else(|| format!("AVR build omitted metric {prefix:?}"))?
        + prefix.len();
    let rest = &report[start..];
    let end = rest
        .find(suffix)
        .ok_or_else(|| format!("AVR build malformed metric {prefix:?}"))?;
    Ok(rest[..end].trim().replace(',', "").parse()?)
}

fn validate_sizes(flash: u64, sram: u64) -> Result<(), Box<dyn std::error::Error>> {
    if flash > MAX_FLASH_BYTES {
        return Err(format!("AVR flash capacity exceeded: {flash} > {MAX_FLASH_BYTES}").into());
    }
    if sram > MAX_STATIC_SRAM_BYTES {
        return Err(format!(
            "AVR static SRAM capacity exceeded after reserving {STACK_RESERVE_BYTES} bytes for stack: {sram} > {MAX_STATIC_SRAM_BYTES}"
        )
        .into());
    }
    Ok(())
}

fn require_success(output: &Output, action: &str) -> Result<(), Box<dyn std::error::Error>> {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{action} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
    .into())
}

fn upload_artifact(
    root: &Path,
    port: &Path,
    artifact: &Path,
    action: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let child = Command::new(avrdude_bin(root))
        .arg(format!("-C{}", avrdude_config(root).display()))
        .args(["-q", "-q", "-patmega32u4", "-cavr109", "-b57600", "-D"])
        .arg(format!("-P{}", port.display()))
        .arg(format!("-Uflash:w:{}:i", artifact.display()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let output = wait_for_child_output(child, AVRDUDE_DEADLINE, action)?;
    require_success(&output, action)
}

fn wait_for_child_output(
    mut child: Child,
    timeout: Duration,
    action: &str,
) -> Result<Output, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait()?.is_some() {
            return Ok(child.wait_with_output()?);
        }
        if Instant::now() >= deadline {
            child.kill()?;
            let output = child.wait_with_output()?;
            return Err(format!(
                "{action} timed out after {} ms; bootloader port may have disappeared: {}",
                timeout.as_millis(),
                String::from_utf8_lossy(&output.stderr).trim()
            )
            .into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

fn git_head(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    require_success(&output, "git HEAD identity")?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn relative(root: &Path, path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(path.strip_prefix(root)?.display().to_string())
}

fn write_receipt<T: Serialize>(
    path: &Path,
    receipt: &T,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(receipt)?;
    fs::write(path, &json)?;
    if opts.json {
        println!("{}", String::from_utf8(json)?);
    } else if !opts.quiet {
        println!("wrote {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
