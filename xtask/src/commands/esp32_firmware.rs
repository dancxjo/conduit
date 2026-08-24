use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{cli::GlobalOpts, workspace::workspace_root};

const PACKAGE_PATH: &str = "firmware/conduit-esp32-wroom-signal";
const DESCRIPTOR_PATH: &str = "firmware/conduit-esp32-wroom-signal/architecture-package.json";

#[derive(Args, Debug)]
pub struct Esp32FirmwareArgs {
    #[command(subcommand)]
    command: Esp32FirmwareCommand,
}

#[derive(Subcommand, Debug)]
enum Esp32FirmwareCommand {
    /// Validate and compile the minimal and full machine-only configurations.
    Check {
        /// Receipt destination, relative to the repository root.
        #[arg(long, default_value = "target/esp32-firmware/check-receipt.json")]
        receipt: PathBuf,
        /// Permit package-input edits while developing; CI must never use this.
        #[arg(long)]
        allow_dirty: bool,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct ArchitecturePackage {
    schema: String,
    package: String,
    revision: u32,
    chip: String,
    board_descriptor: String,
    target: String,
    toolchain: String,
    toolchain_action: String,
    minimal_features: Vec<String>,
    full_features: Vec<String>,
    minimal_bases: Vec<String>,
    full_bases: Vec<String>,
    artifact: String,
}

#[derive(Serialize)]
struct CheckReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    source_sha: String,
    lock_sha256: String,
    architecture_package_sha256: String,
    architecture_package: String,
    architecture_revision: u32,
    toolchain: String,
    target: String,
    chip: String,
    board_descriptor: String,
    minimal_bases: Vec<String>,
    artifact_bases: Vec<String>,
    minimal_runtime_packages: Vec<String>,
    full_runtime_packages: Vec<String>,
    artifact_sha256: Option<String>,
    check_identity: String,
    excluded_truth: [&'static str; 7],
}

pub fn run(args: Esp32FirmwareArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    match args.command {
        Esp32FirmwareCommand::Check {
            receipt,
            allow_dirty,
        } => check(&root, &receipt, allow_dirty, opts),
    }
}

fn check(
    root: &Path,
    receipt_path: &Path,
    allow_dirty: bool,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let package_root = root.join(PACKAGE_PATH);
    let descriptor_bytes = fs::read(root.join(DESCRIPTOR_PATH))?;
    let descriptor: ArchitecturePackage = serde_json::from_slice(&descriptor_bytes)?;
    validate_descriptor(&descriptor)?;
    validate_package_inputs(root, allow_dirty)?;

    let minimal = runtime_packages(&package_root, false)?;
    let full = runtime_packages(&package_root, true)?;
    if !minimal.is_subset(&full) || minimal == full {
        return Err("minimal ESP32 runtime closure must be a strict subset of full closure".into());
    }
    for required in ["esp-radio", "trouble-host"] {
        if minimal.contains(required) || !full.contains(required) {
            return Err(format!("feature closure mapping refused for {required}").into());
        }
    }

    let artifact_sha256 = if opts.dry_run {
        None
    } else {
        cargo_check(&package_root, false)?;
        cargo_build(&package_root)?;
        Some(sha256(&fs::read(package_root.join(&descriptor.artifact))?))
    };

    let source_sha = command_text(root, "git", &["rev-parse", "HEAD"])?;
    let lock_sha256 = sha256(&fs::read(package_root.join("Cargo.lock"))?);
    let architecture_package_sha256 = sha256(&descriptor_bytes);
    let check_identity =
        sha256(format!("{source_sha}:{lock_sha256}:{architecture_package_sha256}").as_bytes());

    let receipt = CheckReceipt {
        schema: "conduit.architecture-package/check-receipt@1",
        outcome: if opts.dry_run { "planned" } else { "compiled" },
        proof_class: "machine-only-contract-compile",
        source_sha: command_text(root, "git", &["rev-parse", "HEAD"])?,
        lock_sha256,
        architecture_package_sha256,
        architecture_package: descriptor.schema,
        architecture_revision: descriptor.revision,
        toolchain: descriptor.toolchain,
        target: descriptor.target,
        chip: descriptor.chip,
        board_descriptor: descriptor.board_descriptor,
        minimal_bases: descriptor.minimal_bases,
        artifact_bases: descriptor.full_bases,
        minimal_runtime_packages: minimal.into_iter().collect(),
        full_runtime_packages: full.into_iter().collect(),
        artifact_sha256,
        check_identity,
        excluded_truth: [
            "physical-boot",
            "host-id",
            "boot-id",
            "host-offer",
            "line",
            "peripheral-readiness",
            "flash-success",
        ],
    };
    let destination = root.join(receipt_path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&destination, serde_json::to_vec_pretty(&receipt)?)?;
    if opts.json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else if !opts.quiet {
        println!("ESP32 MACHINE CHECKED: {}", destination.display());
    }
    Ok(())
}

fn validate_descriptor(value: &ArchitecturePackage) -> Result<(), Box<dyn std::error::Error>> {
    if value.schema != "conduit.architecture-package/esp32-firmware@1"
        || value.package != "conduit-esp32-wroom-signal"
        || value.revision == 0
        || value.chip != "esp32"
        || value.target != "xtensa-esp32-none-elf"
        || value.board_descriptor != "observed/hw-463-esp-wroom-32@1"
        || value.minimal_features != Vec::<String>::new()
        || value.full_features != ["bluetooth"]
        || value.minimal_bases != ["kernel-signal"]
        || value.full_bases != ["kernel-signal", "bluetooth-le-gatt"]
        || value.toolchain != "esp-rs/rust-build@v1.91.1.0"
        || value.toolchain_action
            != "esp-rs/xtensa-toolchain@ec6d36527049a7f4fb2cb0c1a644668c1bb8a2a4"
    {
        return Err("ESP32 architecture package descriptor refused".into());
    }
    Ok(())
}

fn validate_package_inputs(
    root: &Path,
    allow_dirty: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !root.join(PACKAGE_PATH).join("Cargo.lock").is_file() {
        return Err("ESP32 architecture package requires its committed Cargo.lock".into());
    }
    if !allow_dirty {
        let output = Command::new("git")
            .args(["status", "--porcelain", "--", PACKAGE_PATH])
            .current_dir(root)
            .output()?;
        if !output.status.success() || !output.stdout.is_empty() {
            return Err("ESP32 architecture package inputs are dirty; commit them or use --allow-dirty for local development".into());
        }
    }
    Ok(())
}

fn runtime_packages(
    package_root: &Path,
    full: bool,
) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let mut args = vec!["tree", "--locked", "--edges", "normal", "--prefix", "none"];
    if full {
        args.extend(["--no-default-features", "--features", "bluetooth"]);
    } else {
        args.push("--no-default-features");
    }
    let output = command_output(package_root, "cargo", &args)?;
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect())
}

fn cargo_check(package_root: &Path, full: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec![
        "+esp",
        "check",
        "--release",
        "--locked",
        "--no-default-features",
    ];
    if full {
        args.extend(["--features", "bluetooth"]);
    }
    command_output(package_root, "cargo", &args)?;
    Ok(())
}

fn cargo_build(package_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    command_output(
        package_root,
        "cargo",
        &[
            "+esp",
            "build",
            "--release",
            "--locked",
            "--no-default-features",
            "--features",
            "bluetooth",
        ],
    )?;
    Ok(())
}

fn command_text(
    root: &Path,
    program: &str,
    args: &[&str],
) -> Result<String, Box<dyn std::error::Error>> {
    Ok(
        String::from_utf8(command_output(root, program, args)?.stdout)?
            .trim()
            .to_owned(),
    )
}

fn command_output(
    root: &Path,
    program: &str,
    args: &[&str],
) -> Result<Output, Box<dyn std::error::Error>> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "{} {} failed: {}",
            program,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(output)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> ArchitecturePackage {
        serde_json::from_slice(&fs::read(workspace_root().unwrap().join(DESCRIPTOR_PATH)).unwrap())
            .unwrap()
    }

    #[test]
    fn exact_descriptor_is_accepted() {
        validate_descriptor(&descriptor()).unwrap();
    }

    #[test]
    fn wrong_chip_is_refused() {
        let mut value = descriptor();
        value.chip = "esp32-c3".into();
        assert!(validate_descriptor(&value).is_err());
    }

    #[test]
    fn impossible_feature_mapping_is_refused() {
        let mut value = descriptor();
        value.full_features.clear();
        assert!(validate_descriptor(&value).is_err());
    }

    #[test]
    fn stale_toolchain_is_refused() {
        let mut value = descriptor();
        value.toolchain = "esp-rs/rust-build@latest".into();
        assert!(validate_descriptor(&value).is_err());
    }
}
