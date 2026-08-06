use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::commands::pico::PicoArgs;

/// Repository orchestration task runner for Conduit.
#[derive(Parser, Debug)]
#[command(name = "xtask", about = "Conduit repository orchestration")]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOpts,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Clone, Default)]
pub struct GlobalOpts {
    /// Print planned probes or commands without executing them.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Suppress non-error human output.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Emit one structured JSON report to stdout.
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Inspect repository and platform prerequisites.
    Doctor(DoctorArgs),
    /// Build, flash, or verify the Pico W local Signal proof.
    Pico(PicoArgs),
    /// Run the complete Pico W local workflow.
    PicoLocal(PicoArgs),
    /// Run interactive demonstrations.
    Demo(DemoArgs),
}

#[derive(Args, Debug)]
pub struct DemoArgs {
    #[command(subcommand)]
    pub command: DemoCommand,
}

#[derive(Subcommand, Debug)]
pub enum DemoCommand {
    /// Run the S4 distributed toggle proof interactively.
    Toggle,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// What to inspect (default: all).
    #[arg(default_value = "all")]
    pub target: DoctorTarget,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorTarget {
    All,
    Browser,
    Pico,
}

impl DoctorTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Browser => "browser",
            Self::Pico => "pico",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_and_pico_commands_parse() {
        let doctor = Cli::try_parse_from(["xtask", "--dry-run", "doctor", "pico"])
            .expect("doctor command parses");
        assert!(doctor.global.dry_run);
        assert!(matches!(doctor.command, Command::Doctor(_)));

        let pico = Cli::try_parse_from(["xtask", "pico", "build"]).expect("pico command parses");
        assert!(matches!(pico.command, Command::Pico(_)));

        let toggle =
            Cli::try_parse_from(["xtask", "demo", "toggle"]).expect("demo toggle command parses");
        assert!(matches!(
            toggle.command,
            Command::Demo(DemoArgs {
                command: DemoCommand::Toggle
            })
        ));
    }
}
