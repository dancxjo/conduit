use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use clap::{Args, Subcommand};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{cli::GlobalOpts, workspace::workspace_root};

mod avr_toolchain;
mod cdc_verify;
mod hil_observe;

use avr_toolchain::{
    config_path, provision, verify_cores, ARDUINO_AVR_VERSION, CLI_VERSION, SPARKFUN_AVR_VERSION,
};
const FQBN: &str = "SparkFun:avr:promicro:cpu=16MHzatmega32U4";
const SKETCH: &str = "targets/avr/firmware/promicro_brainstem";
const EXPECTED_BY_ID: &str = "usb-SparkFun_SparkFun_Pro_Micro-if00";
const EXPECTED_VID: &str = "1b4f";
const EXPECTED_PID: &str = "9206";
const MAX_FLASH_BYTES: u64 = 28_672;
const MAX_SRAM_BYTES: u64 = 2_560;

#[derive(Args, Debug)]
pub struct AvrArgs {
    #[command(subcommand)]
    command: AvrCommand,
}

#[derive(Subcommand, Debug)]
enum AvrCommand {
    /// Provision and verify the pinned AVR build boundary without hardware access.
    Check,
    /// Build the exact fail-closed Pro Micro image and write a receipt.
    Build {
        /// Compile the guarded Create HIL executor; it still boots isolated.
        #[arg(long)]
        create_hil: bool,
        #[arg(long, default_value = "target/avr-promicro/build-receipt.json")]
        receipt: PathBuf,
    },
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
        /// Flash the guarded HIL artifact rather than the transmitter-free image.
        #[arg(long)]
        create_hil: bool,
        #[arg(long, default_value = "target/avr-promicro/flash-receipt.json")]
        receipt: PathBuf,
    },
    /// Bind and verify one exact fail-closed Boot over guarded USB CDC.
    Verify(cdc_verify::VerifyArgs),
    /// Execute and verify one exact attended Create group-zero observation.
    Observe(hil_observe::ObserveArgs),
}

#[derive(Debug, Serialize)]
struct BuildReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    profile: &'static str,
    source_sha: String,
    target: &'static str,
    board_variant: &'static str,
    arduino_cli: &'static str,
    arduino_avr: &'static str,
    sparkfun_avr: &'static str,
    artifact: String,
    artifact_sha256: String,
    flash_bytes: u64,
    flash_limit: u64,
    sram_bytes: u64,
    sram_limit: u64,
    create_uart: &'static str,
}

#[derive(Debug, Serialize)]
struct FlashReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    profile: &'static str,
    source_sha: String,
    target: &'static str,
    port: String,
    usb_vid: &'static str,
    usb_pid: &'static str,
    artifact_sha256: String,
    create_stopped: bool,
    attended: bool,
    wheels_clear: bool,
    create_uart: &'static str,
}

pub fn run(args: AvrArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        AvrCommand::Check => run_check(opts),
        AvrCommand::Build {
            create_hil,
            receipt,
        } => run_build(&receipt, create_hil, opts).map(|_| ()),
        AvrCommand::Flash {
            port,
            artifact_sha256,
            create_stopped,
            attended,
            wheels_clear,
            create_hil,
            receipt,
        } => run_flash(
            &port,
            &artifact_sha256,
            PhysicalGate {
                create_stopped,
                attended,
                wheels_clear,
            },
            create_hil,
            &receipt,
            opts,
        ),
        AvrCommand::Verify(args) => cdc_verify::run(args, opts),
        AvrCommand::Observe(args) => hil_observe::run(args, opts),
    }
}

fn run_check(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    if opts.dry_run {
        if !opts.quiet {
            println!("would provision and verify Arduino CLI {CLI_VERSION}, Arduino AVR {ARDUINO_AVR_VERSION}, and SparkFun AVR {SPARKFUN_AVR_VERSION}");
        }
        return Ok(());
    }
    let cli = provision(&root)?;
    verify_cores(&cli, &root)?;
    if !opts.quiet {
        println!("AVR boundary ready: {FQBN}");
    }
    Ok(())
}

fn run_build(
    receipt: &Path,
    create_hil: bool,
    opts: &GlobalOpts,
) -> Result<(PathBuf, String), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let output_dir = root.join(if create_hil {
        "target/avr-promicro/build-hil"
    } else {
        "target/avr-promicro/build"
    });
    let artifact = output_dir.join("promicro_brainstem.ino.hex");
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would build {FQBN} profile={} from {}",
                if create_hil { "create-hil" } else { "isolated" },
                root.join(SKETCH).display()
            );
        }
        return Ok((artifact, String::new()));
    }
    validate_sketch_layout(&root.join(SKETCH))?;
    let cli = provision(&root)?;
    verify_cores(&cli, &root)?;
    fs::create_dir_all(&output_dir)?;
    let mut command = Command::new(&cli);
    command
        .args([
            "compile",
            "--fqbn",
            FQBN,
            "--warnings",
            "all",
            "--build-path",
        ])
        .arg(&output_dir)
        .arg(root.join(SKETCH));
    if create_hil {
        command.args([
            "--build-property",
            "compiler.cpp.extra_flags=-DCONDUIT_CREATE_HIL=1",
        ]);
    }
    let output = command
        .args(["--config-file"])
        .arg(config_path(&root))
        .output()?;
    require_success(&output, "AVR firmware build")?;
    if !artifact.is_file() {
        return Err(format!("AVR build omitted {}", artifact.display()).into());
    }
    let report = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let flash_bytes = metric(&report, "Sketch uses ", " bytes")?;
    let sram_bytes = metric(&report, "Global variables use ", " bytes")?;
    validate_sizes(flash_bytes, sram_bytes)?;
    let digest = sha256_file(&artifact)?;
    let record = BuildReceipt {
        schema: "conduit.avr-promicro/build@1",
        outcome: "built",
        proof_class: "machine-only-contract-compile",
        profile: if create_hil { "create-hil" } else { "isolated" },
        source_sha: git_head(&root)?,
        target: FQBN,
        board_variant: "atmega32u4-5v-16mhz-usb-pid-9206",
        arduino_cli: CLI_VERSION,
        arduino_avr: ARDUINO_AVR_VERSION,
        sparkfun_avr: SPARKFUN_AVR_VERSION,
        artifact: relative(&root, &artifact)?,
        artifact_sha256: digest.clone(),
        flash_bytes,
        flash_limit: MAX_FLASH_BYTES,
        sram_bytes,
        sram_limit: MAX_SRAM_BYTES,
        create_uart: if create_hil {
            "isolated-until-exact-execution"
        } else {
            "isolated-no-transmitter"
        },
    };
    write_receipt(&root.join(receipt), &record, opts)?;
    Ok((artifact, digest))
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
    create_hil: bool,
    receipt: &Path,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_flash_request(port, expected_digest, gate)?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would verify {EXPECTED_BY_ID}, rebuild profile={}, verify its digest, and flash the boot-isolated image",
                if create_hil { "create-hil" } else { "isolated" }
            );
        }
        return Ok(());
    }
    verify_device(port)?;
    let (artifact, digest) = run_build(
        Path::new(if create_hil {
            "target/avr-promicro/build-hil-receipt.json"
        } else {
            "target/avr-promicro/build-receipt.json"
        }),
        create_hil,
        opts,
    )?;
    if digest != expected_digest {
        return Err(format!(
            "AVR artifact digest mismatch: expected {expected_digest}, built {digest}"
        )
        .into());
    }
    let root = workspace_root()?;
    let cli = provision(&root)?;
    let output = Command::new(cli)
        .args(["upload", "--fqbn", FQBN, "--port"])
        .arg(port)
        .args(["--input-dir"])
        .arg(artifact.parent().ok_or("AVR artifact has no parent")?)
        .args(["--config-file"])
        .arg(config_path(&root))
        .output()?;
    require_success(&output, "guarded AVR flash")?;
    let record = FlashReceipt {
        schema: "conduit.avr-promicro/flash@1",
        outcome: "flashed",
        proof_class: "physical-flash-no-cdc-open",
        profile: if create_hil { "create-hil" } else { "isolated" },
        source_sha: git_head(&root)?,
        target: FQBN,
        port: port.display().to_string(),
        usb_vid: EXPECTED_VID,
        usb_pid: EXPECTED_PID,
        artifact_sha256: digest,
        create_stopped: gate.create_stopped,
        attended: gate.attended,
        wheels_clear: gate.wheels_clear,
        create_uart: if create_hil {
            "isolated-until-exact-execution"
        } else {
            "isolated-no-transmitter"
        },
    };
    write_receipt(&root.join(receipt), &record, opts)
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
    Ok(rest[..end].replace(',', "").parse()?)
}

fn validate_sizes(flash: u64, sram: u64) -> Result<(), Box<dyn std::error::Error>> {
    if flash > MAX_FLASH_BYTES {
        return Err(format!("AVR flash capacity exceeded: {flash} > {MAX_FLASH_BYTES}").into());
    }
    if sram > MAX_SRAM_BYTES {
        return Err(format!("AVR SRAM capacity exceeded: {sram} > {MAX_SRAM_BYTES}").into());
    }
    Ok(())
}

fn validate_sketch_layout(sketch: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(sketch)? {
        let path = entry?.path();
        if path.is_file() && source_replaces_arduino_entry(&path) {
            return Err(format!(
                "AVR sketch contains standalone source {}: host tests must remain outside the sketch so their main() cannot replace Arduino setup()/loop()",
                path.display()
            )
            .into());
        }
    }
    Ok(())
}

fn source_replaces_arduino_entry(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("c" | "cc" | "cpp" | "cxx")
    )
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
