//! A controlled loopback route selected by a fresh installed owner before Boot.

use std::{path::PathBuf, process::Command};

use clap::Args as ClapArgs;

use crate::cli::GlobalOpts;

use super::{profile::Paths, ConduitosArch, ConduitosError};

#[derive(ClapArgs, Debug)]
pub(super) struct Args {
    /// Existing local Ollama origin; no model is downloaded or started.
    #[arg(long)]
    upstream: String,
    /// New socket under a private directory, retained while this command runs.
    #[arg(long)]
    control_socket: PathBuf,
}

pub(super) fn execute(args: &Args, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "owner-model-route-requires-live-provider",
            "the controlled route must connect to an existing local provider",
        ));
    }
    let root = Paths::new(ConduitosArch::X86_64)?.root;
    let script = root.join("proof/browser/local-model-route.mjs");
    if !script.is_file() {
        return Err(ConduitosError::refusal(
            "owner-model-route-script-unavailable",
            format!("route script is unavailable: {}", script.display()),
        ));
    }
    if args.control_socket.exists() {
        return Err(ConduitosError::refusal(
            "owner-model-route-socket-exists",
            "provide a new private control socket path",
        ));
    }
    let parent = args.control_socket.parent().ok_or_else(|| {
        ConduitosError::refusal(
            "owner-model-route-directory",
            "control socket needs a parent",
        )
    })?;
    let metadata = std::fs::metadata(parent).map_err(|error| {
        ConduitosError::refusal("owner-model-route-directory", error.to_string())
    })?;
    if !metadata.is_dir() {
        return Err(ConduitosError::refusal(
            "owner-model-route-directory",
            "control socket parent must be a private directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(ConduitosError::refusal(
                "owner-model-route-directory",
                "control socket parent must exclude group and other access",
            ));
        }
    }
    let status = Command::new("node")
        .arg(script)
        .arg(&args.upstream)
        .arg(&args.control_socket)
        .current_dir(root)
        .status()
        .map_err(|error| ConduitosError::refusal("owner-model-route-launch", error.to_string()))?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "owner-model-route-failed",
            format!("controlled route exited with {status}"),
        ));
    }
    Ok(())
}
