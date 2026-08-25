use std::{path::PathBuf, process::Command};

use clap::{Args, Subcommand};

use crate::{cli::GlobalOpts, workspace::workspace_root};

const ARCHITECTURE_RUNNER_MANIFEST: &str =
    "firmware/conduit-esp32-wroom-signal/architecture-package-runner/Cargo.toml";

#[derive(Args, Debug)]
pub struct Esp32FirmwareArgs {
    #[command(subcommand)]
    command: Esp32FirmwareCommand,
}

#[derive(Subcommand, Debug)]
enum Esp32FirmwareCommand {
    /// Delegate the locked machine-only check to the ESP32 architecture package.
    Check {
        /// Receipt destination, relative to the repository root.
        #[arg(long, default_value = "target/esp32-firmware/check-receipt.json")]
        receipt: PathBuf,
        /// Permit repository input edits while developing; CI must never use this.
        #[arg(long)]
        allow_dirty: bool,
    },
}

pub fn run(args: Esp32FirmwareArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let Esp32FirmwareCommand::Check {
        receipt,
        allow_dirty,
    } = args.command;
    let receipt = root.join(receipt);
    let manifest = root.join(ARCHITECTURE_RUNNER_MANIFEST);
    if !manifest.is_file() {
        return Err(format!(
            "ESP32 architecture-package runner is unavailable at {}",
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
        .arg(receipt);
    if allow_dirty {
        command.arg("--allow-dirty");
    }
    if opts.dry_run {
        command.arg("--dry-run");
    }
    if opts.quiet {
        command.arg("--quiet");
    }
    if opts.json {
        command.arg("--json");
    }

    let status = command.status()?;
    if !status.success() {
        return Err(
            format!("ESP32 architecture-package runner failed with status {status}").into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_xtask_names_only_the_architecture_runner_boundary() {
        assert_eq!(
            ARCHITECTURE_RUNNER_MANIFEST,
            "firmware/conduit-esp32-wroom-signal/architecture-package-runner/Cargo.toml"
        );
    }
}
