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

    /// Forward --locked to Cargo commands.
    #[arg(long, global = true)]
    pub locked: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Execute repository validation check suites.
    Check(CheckArgs),
    /// Execute platform and protocol proof suites.
    Prove(ProveArgs),
    /// Print the versioned machine-readable proof command contract.
    Proofs(ProofsArgs),
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
pub struct CheckArgs {
    /// Which check suite to execute (default: workspace).
    #[arg(default_value = "workspace")]
    pub suite: CheckSuite,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckSuite {
    Workspace,
    Browser,
    BrowserHost,
    Sim,
    KernelTakeover,
    PlanningS2,
    FormS3,
    Realm,
    Observatory,
    StdCatalog,
    All,
}

#[derive(Args, Debug)]
pub struct ProveArgs {
    /// Which proof suite to execute.
    pub proof: ProveTarget,

    /// Explicit USB CDC link port (CDC 0).
    #[arg(long)]
    pub link_port: Option<String>,

    /// Explicit USB CDC evidence port (CDC 1).
    #[arg(long)]
    pub evidence_port: Option<String>,

    /// Run interactive button console control mode.
    #[arg(long)]
    pub interactive: bool,

    /// Corrupt the first planned Signal after kernel emission and require an
    /// honest two-sided sink-failure terminal instead of success.
    #[arg(long)]
    pub induce_sink_failure: bool,
}

#[derive(Args, Debug)]
pub struct ProofsArgs {
    /// Validate one JSON proof record against its exact registered command contract.
    #[arg(long)]
    pub validate_record: Option<std::path::PathBuf>,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProveTarget {
    StdBrowserS4,
    StdBrowserToggle,
    BrowserHost,
    StdPicoUsb,
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

        let pico_build_remote = Cli::try_parse_from(["xtask", "pico", "build", "--usb-remote"])
            .expect("pico build --usb-remote parses");
        if let Command::Pico(args) = pico_build_remote.command {
            assert!(args.usb_remote);
        } else {
            panic!("expected Command::Pico");
        }

        let pico_flash_remote = Cli::try_parse_from(["xtask", "pico", "flash", "--usb-remote"])
            .expect("pico flash --usb-remote parses");
        if let Command::Pico(args) = pico_flash_remote.command {
            assert!(args.usb_remote);
        } else {
            panic!("expected Command::Pico");
        }

        let toggle =
            Cli::try_parse_from(["xtask", "demo", "toggle"]).expect("demo toggle command parses");
        assert!(matches!(
            toggle.command,
            Command::Demo(DemoArgs {
                command: DemoCommand::Toggle
            })
        ));

        let check =
            Cli::try_parse_from(["xtask", "check", "workspace"]).expect("check command parses");
        assert!(matches!(check.command, Command::Check(_)));

        let prove = Cli::try_parse_from(["xtask", "prove", "std-browser-s4"])
            .expect("prove command parses");
        assert!(matches!(prove.command, Command::Prove(_)));

        let proofs = Cli::try_parse_from(["xtask", "--json", "proofs"])
            .expect("proof catalog command parses");
        assert!(proofs.global.json);
        assert!(matches!(proofs.command, Command::Proofs(_)));
    }
}
