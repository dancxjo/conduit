use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use clap::{Args, Subcommand};
use conduit_host_esp32_fabrication::Esp32FamilyTarget;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{cli::GlobalOpts, workspace::workspace_root};

mod browser_release;
mod morse_key;

const FABRICATION_RUNNER_MANIFEST: &str =
    "targets/esp32/firmware/wroom-signal/fabrication-package-runner/Cargo.toml";
const ESPFLASH_VERSION: &str = "4.5.0";
const ESPFLASH_ARCHIVE: &str = "espflash-x86_64-unknown-linux-gnu.zip";
const ESPFLASH_URL: &str = "https://github.com/esp-rs/espflash/releases/download/v4.5.0/espflash-x86_64-unknown-linux-gnu.zip";
const ESPFLASH_ARCHIVE_SHA256: &str =
    "dcd7fd822f247df18966935bbb10c7cb4b700c87172349633e9e323ee2542717";
const ESPFLASH_BINARY_SHA256: &str =
    "03869e52b4ab6433720087eb8b895a94852e0c6f1e44b07c184b1dfeaab2f6ae";

#[derive(Args, Debug)]
pub struct Esp32FirmwareArgs {
    #[command(subcommand)]
    command: Esp32FirmwareCommand,
}

#[derive(Subcommand, Debug)]
enum Esp32FirmwareCommand {
    /// Delegate the locked machine-only check to the ESP32 fabrication package.
    Check {
        #[arg(long, default_value = "target/esp32-firmware/check-receipt.json")]
        receipt: PathBuf,
        #[arg(long)]
        allow_dirty: bool,
    },
    /// Build one exact descriptor-bound ESP32-family firmware image.
    Build {
        #[arg(long)]
        target: Esp32FamilyTarget,
        /// Build the exact distributed-Lenia worker image.
        #[arg(long)]
        distributed_lenia: bool,
        #[arg(long, default_value = "target/esp32-firmware/build-receipt.json")]
        receipt: PathBuf,
        /// Also seal one merged browser-deployable generic release IMAGE and sidecar manifest.
        #[arg(long)]
        browser_artifact: Option<PathBuf>,
    },
    /// Build and flash one exact attached ESP32-family target.
    Flash {
        #[arg(long)]
        target: Esp32FamilyTarget,
        /// Flash the exact distributed-Lenia worker image.
        #[arg(long)]
        distributed_lenia: bool,
        #[arg(long)]
        port: PathBuf,
        /// Exact USB serial expected for the selected target.
        #[arg(long)]
        confirm_serial: String,
        #[arg(long, default_value = "target/esp32-firmware/flash-receipt.json")]
        receipt: PathBuf,
    },
    /// Flash and physically prove the attached C3 BOOT button as a Morse key.
    MorseKey(morse_key::MorseKeyArgs),
}

fn artifact(target: Esp32FamilyTarget, root: &Path) -> PathBuf {
    let facts = target.facts();
    root.join(facts.package_dir)
        .join("target")
        .join(facts.cargo_target)
        .join("release")
        .join(facts.artifact_name)
}

#[derive(Serialize)]
struct FirmwareReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    target: Esp32FamilyTarget,
    firmware_mode: &'static str,
    source_sha: String,
    artifact: String,
    artifact_sha256: String,
    tool: Option<ToolReceipt>,
    serial_path: Option<String>,
    usb_serial: Option<String>,
}

#[derive(Serialize)]
struct ToolReceipt {
    name: &'static str,
    version: &'static str,
    archive_sha256: &'static str,
    binary_sha256: &'static str,
}

pub fn run(args: Esp32FirmwareArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        Esp32FirmwareCommand::Check {
            receipt,
            allow_dirty,
        } => run_check(receipt, allow_dirty, opts),
        Esp32FirmwareCommand::Build {
            target,
            distributed_lenia,
            receipt,
            browser_artifact,
        } => run_build(
            target,
            distributed_lenia,
            receipt,
            browser_artifact.as_deref(),
            opts,
        )
        .map(|_| ()),
        Esp32FirmwareCommand::Flash {
            target,
            distributed_lenia,
            port,
            confirm_serial,
            receipt,
        } => run_flash(
            target,
            distributed_lenia,
            &port,
            &confirm_serial,
            receipt,
            opts,
        ),
        Esp32FirmwareCommand::MorseKey(args) => morse_key::run(args, opts),
    }
}

fn run_check(
    receipt: PathBuf,
    allow_dirty: bool,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let manifest = root.join(FABRICATION_RUNNER_MANIFEST);
    if !manifest.is_file() {
        return Err(format!(
            "ESP32 fabrication-package runner is unavailable at {}",
            manifest.display()
        )
        .into());
    }
    let mut command = Command::new("cargo");
    command
        .current_dir(&root)
        .args(["run", "--locked", "--manifest-path"])
        .arg(&manifest)
        .args(["--", "check", "--repo-root"])
        .arg(&root)
        .arg("--receipt")
        .arg(root.join(receipt));
    if allow_dirty {
        command.arg("--allow-dirty");
    }
    forward_output_flags(&mut command, opts);
    require_success(command, "ESP32 fabrication-package runner")
}

fn run_build(
    target: Esp32FamilyTarget,
    distributed_lenia: bool,
    receipt: PathBuf,
    browser_artifact: Option<&Path>,
    opts: &GlobalOpts,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let facts = target.facts();
    let package = root.join(facts.package_dir);
    if !package.join("Cargo.toml").is_file() {
        return Err(format!("ESP32 package is unavailable at {}", package.display()).into());
    }
    let artifact = artifact(target, &root);
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would build {} with +{} from {}",
                facts.artifact_name,
                facts.rust_toolchain,
                package.display()
            );
        }
        return Ok(artifact);
    }
    let mut command = Command::new("cargo");
    command
        .current_dir(&package)
        .arg(format!("+{}", facts.rust_toolchain))
        .args(["build", "--release", "--features"])
        .arg(if distributed_lenia {
            "distributed-lenia"
        } else {
            "bluetooth"
        });
    if opts.locked {
        command.arg("--locked");
    }
    require_success(command, "ESP32 firmware build")?;
    if !artifact.is_file() {
        return Err(format!("ESP32 build omitted artifact {}", artifact.display()).into());
    }
    let record = FirmwareReceipt {
        schema: "conduit.esp32-firmware/build@1",
        outcome: "built",
        proof_class: "machine-only-contract-compile",
        target,
        firmware_mode: if distributed_lenia {
            "distributed-lenia"
        } else {
            "bluetooth"
        },
        source_sha: git_head(&root)?,
        artifact: relative(&root, &artifact)?,
        artifact_sha256: sha256_file(&artifact)?,
        tool: None,
        serial_path: None,
        usb_serial: None,
    };
    write_receipt(&root.join(receipt), &record, opts)?;
    if let Some(output) = browser_artifact {
        browser_release::write(&root, target, &artifact, output, &record.source_sha, opts)?;
    }
    Ok(artifact)
}

fn run_flash(
    target: Esp32FamilyTarget,
    distributed_lenia: bool,
    port: &Path,
    confirm_serial: &str,
    receipt: PathBuf,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let facts = target.facts();
    if confirm_serial != facts.usb_serial {
        return Err(format!("ESP32 flash confirmation mismatch: target requires USB serial {}, received {confirm_serial}", facts.usb_serial).into());
    }
    if opts.dry_run {
        let _ = run_build(target, distributed_lenia, PathBuf::new(), None, opts)?;
        if !opts.quiet {
            println!(
                "would flash {} through {} after verifying USB serial {}",
                facts.artifact_name,
                port.display(),
                confirm_serial
            );
        }
        return Ok(());
    }
    let observed = serial_properties(port)?;
    if observed.get("ID_SERIAL_SHORT").map(String::as_str) != Some(confirm_serial) {
        return Err(format!(
            "ESP32 serial identity mismatch at {}: expected {confirm_serial}, observed {}",
            port.display(),
            observed
                .get("ID_SERIAL_SHORT")
                .map(String::as_str)
                .unwrap_or("<missing>")
        )
        .into());
    }
    let artifact = run_build(
        target,
        distributed_lenia,
        PathBuf::from("target/esp32-firmware/build-receipt.json"),
        None,
        opts,
    )?;
    let root = workspace_root()?;
    let tool = provision_espflash(&root)?;
    let mut command = Command::new(&tool);
    command
        .arg("flash")
        .args(["--chip", facts.espflash_chip, "--port"])
        .arg(port)
        .args(["--non-interactive", "--skip-update-check"])
        .arg(&artifact);
    require_success(command, "verified ESP32 flash")?;
    let record = FirmwareReceipt {
        schema: "conduit.esp32-firmware/flash@1",
        outcome: "flashed-and-verified",
        proof_class: "physical-flash",
        target,
        firmware_mode: if distributed_lenia {
            "distributed-lenia"
        } else {
            "bluetooth"
        },
        source_sha: git_head(&root)?,
        artifact: relative(&root, &artifact)?,
        artifact_sha256: sha256_file(&artifact)?,
        tool: Some(ToolReceipt {
            name: "espflash",
            version: ESPFLASH_VERSION,
            archive_sha256: ESPFLASH_ARCHIVE_SHA256,
            binary_sha256: ESPFLASH_BINARY_SHA256,
        }),
        serial_path: Some(port.display().to_string()),
        usb_serial: Some(confirm_serial.to_owned()),
    };
    write_receipt(&root.join(receipt), &record, opts)
}

fn provision_espflash(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
        return Err("pinned espflash bundle currently supports x86_64 Linux only".into());
    }
    let directory = root.join(format!("target/esp32-tools/espflash-{ESPFLASH_VERSION}"));
    let archive = root.join(format!("target/esp32-tools/{ESPFLASH_ARCHIVE}"));
    let binary = directory.join("espflash");
    if binary.is_file() && sha256_file(&binary)? == ESPFLASH_BINARY_SHA256 {
        return Ok(binary);
    }
    fs::create_dir_all(&directory)?;
    let mut download = Command::new("curl");
    download
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            ESPFLASH_URL,
            "--output",
        ])
        .arg(&archive);
    require_success(download, "pinned espflash download")?;
    let found = sha256_file(&archive)?;
    if found != ESPFLASH_ARCHIVE_SHA256 {
        return Err(format!(
            "espflash archive digest mismatch: expected {ESPFLASH_ARCHIVE_SHA256}, found {found}"
        )
        .into());
    }
    let mut extract = Command::new("unzip");
    extract.args(["-o"]).arg(&archive).arg("-d").arg(&directory);
    require_success(extract, "pinned espflash extraction")?;
    let found = sha256_file(&binary)?;
    if found != ESPFLASH_BINARY_SHA256 {
        return Err(format!(
            "espflash binary digest mismatch: expected {ESPFLASH_BINARY_SHA256}, found {found}"
        )
        .into());
    }
    Ok(binary)
}

fn serial_properties(port: &Path) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let output = Command::new("udevadm")
        .args(["info", "--query=property", "--name"])
        .arg(port)
        .output()?;
    if !output.status.success() {
        return Err(format!("udevadm refused ESP32 serial path {}", port.display()).into());
    }
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect())
}

fn forward_output_flags(command: &mut Command, opts: &GlobalOpts) {
    if opts.dry_run {
        command.arg("--dry-run");
    }
    if opts.quiet {
        command.arg("--quiet");
    }
    if opts.json {
        command.arg("--json");
    }
}
fn require_success(mut command: Command, purpose: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("{purpose} failed with status {status}").into());
    }
    Ok(())
}
fn write_receipt<T: Serialize>(
    path: &Path,
    receipt: &T,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(receipt)?)?;
    if opts.json {
        println!("{}", serde_json::to_string(receipt)?);
    } else if !opts.quiet {
        println!("ESP32 firmware receipt: {}", path.display());
    }
    Ok(())
}
fn git_head(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("failed to resolve source commit".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
fn relative(root: &Path, path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(path
        .strip_prefix(root)?
        .to_str()
        .ok_or("artifact path is not UTF-8")?
        .to_owned())
}
fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_targets_select_reviewed_packages_tools_and_serials() {
        assert_eq!(Esp32FamilyTarget::Wroom.facts().usb_serial, "0001");
        assert_eq!(
            Esp32FamilyTarget::C3.facts().usb_serial,
            "dcf8355da19ded11a7205f84e259fb3e"
        );
        assert_eq!(Esp32FamilyTarget::S3.facts().usb_serial, "54E2006398");
        assert_eq!(Esp32FamilyTarget::C3.facts().rust_toolchain, "1.91.1");
        assert_eq!(
            Esp32FamilyTarget::C3.facts().cargo_target,
            "riscv32imc-unknown-none-elf"
        );
        assert_eq!(Esp32FamilyTarget::C3.facts().espflash_chip, "esp32c3");
        assert_eq!(
            Esp32FamilyTarget::S3.facts().rust_toolchain,
            "esp-conduit-1.91.1"
        );
    }
}
