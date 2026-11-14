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
    /// Apply one exact locally reviewed Source revision while the service is stopped.
    AdoptSource {
        /// Owner-selected File, exact Body basis, entries and qualified workspace selection.
        selection: PathBuf,
        /// Measure the same funded preparation without publishing an owner, joining, or committing Source.
        #[arg(long)]
        prepare_only: bool,
        #[arg(long)]
        state_dir: PathBuf,
    },
    /// Explicitly recheck retained File preparation for the installed release; never adopts Source.
    ReselectSource {
        /// Reviewed old policy/release digests and exact current Body basis.
        request: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
    },
    /// Verify and install one reviewed release bundle without replacing durable identity.
    Install {
        manifest: PathBuf,
        /// Replacement checked Make IMAGE for the retained Host source.
        #[arg(long)]
        host_image: Option<PathBuf>,
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
    /// Prepare optional Linux boot-to-display review artifacts; does not activate services.
    PrepareDirectDisplay {
        /// Exact installed release manifest, used to reverify executable custody.
        manifest: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
        /// New directory for the private unit and human-reviewed activation/restore recipe.
        #[arg(long)]
        output: PathBuf,
        /// Existing unprivileged Linux account that owns the installed Host state.
        #[arg(long)]
        user: String,
        /// Dedicated local virtual terminal (1 through 63).
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=63))]
        tty: u8,
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
    fn direct_display_is_explicit_and_requires_dedicated_session() {
        let base = [
            "conduit",
            "host",
            "service",
            "prepare-direct-display",
            "release.json",
            "--state-dir",
            "state",
            "--output",
            "review",
            "--user",
            "conduit",
        ];
        assert!(Cli::try_parse_from(base).is_err());
        assert!(Cli::try_parse_from(base.into_iter().chain(["--tty", "0"])).is_err());
        assert!(Cli::try_parse_from(base.into_iter().chain(["--tty", "64"])).is_err());
        assert!(matches!(
            Cli::try_parse_from(base.into_iter().chain(["--tty", "7"]))
                .unwrap()
                .command,
            Some(Command::Host {
                command: Some(HostCommand::Service {
                    command: HostServiceCommand::PrepareDirectDisplay { tty: 7, .. }
                })
            })
        ));
    }
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
