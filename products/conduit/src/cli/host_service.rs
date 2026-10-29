//! Installed Host lifecycle commands.
use super::{InstalledModelOptions, InstalledSpeechOptions, InstalledTodoCheckpointOptions};
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
// This one-shot command keeps its reviewed speech and model selections explicit
// for Clap's flattened arguments; the enum is not retained in a running Play.
#[allow(clippy::large_enum_variant)]
pub(crate) enum HostServiceCommand {
    /// Retain checked Host initialization for the next Boot; requires a stopped service.
    Configure {
        #[arg(required_unless_present = "clear", conflicts_with = "clear")]
        configuration: Option<PathBuf>,
        /// Explicitly remove retained checked initialization.
        #[arg(long)]
        clear: bool,
        #[arg(long)]
        state_dir: PathBuf,
    },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command, HostCommand};
    use clap::Parser;
    #[test]
    fn configuration_requires_explicit_source_or_removal() {
        let base = [
            "conduit",
            "host",
            "service",
            "configure",
            "--state-dir",
            "state",
        ];
        assert!(Cli::try_parse_from(base).is_err());
        assert!(
            Cli::try_parse_from(base.into_iter().chain(["source.host.conduit", "--clear"]))
                .is_err()
        );
        assert!(matches!(
            Cli::try_parse_from(base.into_iter().chain(["source.host.conduit"]))
                .unwrap()
                .command,
            Some(Command::Host {
                command: Some(HostCommand::Service {
                    command: HostServiceCommand::Configure {
                        configuration: Some(_),
                        clear: false,
                        ..
                    }
                })
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(base.into_iter().chain(["--clear"]))
                .unwrap()
                .command,
            Some(Command::Host {
                command: Some(HostCommand::Service {
                    command: HostServiceCommand::Configure {
                        configuration: None,
                        clear: true,
                        ..
                    }
                })
            })
        ));
    }
}
