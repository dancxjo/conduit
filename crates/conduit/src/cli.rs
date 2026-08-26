use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Product command-line entrance for installed Conduit workflows.
#[derive(Debug, Parser)]
#[command(name = "conduit", about = "Run and inspect Conduit Forms")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Enter the current Body through the shared Patchbay front door.
    Patchbay {
        /// Select the Host realization used to manifest Patchbay.
        #[arg(long, value_enum, default_value_t = PatchbayHost::Native)]
        on: PatchbayHost,
    },
    /// Check, plan, admit, and execute a Form on available local Hosts.
    Run {
        /// Authored Form to execute.
        form: PathBuf,
        /// Optional exact placement constraints.
        #[arg(long)]
        placements: Option<PathBuf>,
        /// Write a neutral runtime report after execution.
        #[arg(long)]
        report: Option<PathBuf>,
        /// Exact canonical Body construction source used for Host and Line truth.
        #[arg(long)]
        body: Option<PathBuf>,
    },
    /// Check, inspect, or build canonical Host construction truth.
    Host {
        #[command(subcommand)]
        command: HostCommand,
    },
    /// Check, inspect, or build canonical Body construction truth.
    Body {
        #[command(subcommand)]
        command: BodyCommand,
    },
    /// Check a Form and render owned diagnostics without executing it.
    Check {
        form: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Inspect retained Conduit artifacts without executing work.
    Inspect {
        #[command(subcommand)]
        command: InspectCommand,
    },
    /// Run the protected local file-copy task.
    Copy {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<String>,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub(crate) enum PatchbayHost {
    Native,
    Browser,
}

#[derive(Debug, Subcommand)]
pub(crate) enum HostCommand {
    Check {
        source: PathBuf,
    },
    Show {
        source: PathBuf,
    },
    Build {
        source: PathBuf,
        #[arg(long, default_value = "target/host-build")]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum BodyCommand {
    Check {
        source: PathBuf,
    },
    Show {
        source: PathBuf,
    },
    Build {
        source: PathBuf,
        #[arg(long, default_value = "target/body-build")]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum InspectCommand {
    /// Render a neutral runtime report.
    RuntimeReport { report: PathBuf },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_command_tree_parses() {
        assert!(matches!(
            Cli::try_parse_from(["conduit", "patchbay", "--on", "browser"])
                .expect("Patchbay browser entrance parses")
                .command,
            Command::Patchbay {
                on: PatchbayHost::Browser
            }
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "run", "hello.conduit"])
                .expect("run command parses")
                .command,
            Command::Run { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "run",
                "signal.conduit",
                "--body",
                "current.body.conduit"
            ])
            .expect("Body-backed product run parses")
            .command,
            Command::Run { body: Some(_), .. }
        ));
        assert!(Cli::try_parse_from([
            "conduit",
            "run",
            "signal.conduit",
            "--execution-fixture",
            "two-std-line"
        ])
        .is_err());
        assert!(matches!(
            Cli::try_parse_from(["conduit", "check", "hello.conduit", "--json"])
                .expect("check command parses")
                .command,
            Command::Check { json: true, .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "inspect", "runtime-report", "run.json"])
                .expect("inspect command parses")
                .command,
            Command::Inspect { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "host", "build", "linux.host.conduit"])
                .expect("Host build entrance parses")
                .command,
            Command::Host {
                command: HostCommand::Build { .. }
            }
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "body", "show", "current.body.conduit"])
                .expect("Body show entrance parses")
                .command,
            Command::Body {
                command: BodyCommand::Show { .. }
            }
        ));
    }
}
