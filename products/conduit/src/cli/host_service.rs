//! Installed Host service command parsing.
use super::{InstalledModelOptions, InstalledSpeechOptions, InstalledTodoCheckpointOptions};
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
// This one-shot command keeps its reviewed speech and model selections explicit
// for Clap's flattened arguments; the enum is not retained in a running Play.
#[allow(clippy::large_enum_variant)]
pub(crate) enum HostServiceCommand {
    /// Verify and install one reviewed release bundle without replacing durable identity.
    Install {
        manifest: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
        /// Install without activating the service, for foreground Body ownership.
        #[arg(long)]
        no_start: bool,
        #[command(flatten)]
        speech: InstalledSpeechOptions,
        #[command(flatten)]
        model: InstalledModelOptions,
        #[command(flatten)]
        todo_checkpoint: InstalledTodoCheckpointOptions,
    },
    /// Run the durable host in the foreground for a platform service manager.
    Run {
        #[arg(long)]
        state_dir: PathBuf,
        /// Run native Crèche in the retained Body using explicit Host grant policy.
        #[arg(long)]
        creche: Option<PathBuf>,
    },
    /// Print the retained host identity and current runtime status.
    Status {
        #[arg(long)]
        state_dir: PathBuf,
        /// Emit bounded machine-readable Host, Boot, and release identity truth.
        #[arg(long)]
        json: bool,
    },
    /// Make this durable host retain one exact validated Body biography.
    #[command(hide = true)]
    OwnBody {
        /// Exported `conduit.body/biography-evidence@2` document.
        evidence: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
    },
}
