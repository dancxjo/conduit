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
        /// Select a bounded machine-only product execution fixture.
        #[arg(long, value_enum)]
        execution_fixture: Option<ExecutionFixture>,
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub(crate) enum ExecutionFixture {
    TwoStdLine,
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
                "--execution-fixture",
                "two-std-line"
            ])
            .expect("two-std product fixture parses")
            .command,
            Command::Run {
                execution_fixture: Some(ExecutionFixture::TwoStdLine),
                ..
            }
        ));
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
    }
}
