use clap::{Parser, Subcommand};
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
    /// Execute the current typed multi-value kernel profile.
    #[command(hide = true)]
    KernelMultivalue { form: PathBuf },
    /// Compatibility spelling for `conduit check`.
    #[command(hide = true)]
    DiagnoseForm {
        form: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Compatibility spelling for `conduit inspect runtime-report`.
    #[command(hide = true)]
    ObservatoryReport { report: PathBuf },
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
            Cli::try_parse_from(["conduit", "run", "hello.form"])
                .expect("run command parses")
                .command,
            Command::Run { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "check", "hello.form", "--json"])
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
