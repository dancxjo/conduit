use clap::{Args, Parser, Subcommand, ValueEnum};

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
    /// Print commands without executing them.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Increase verbosity.
    #[arg(long, global = true)]
    pub verbose: bool,

    /// Suppress non-error output.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Continue executing steps after a failure.
    #[arg(long, global = true)]
    pub keep_going: bool,

    /// Emit a structured JSON report to stdout.
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Inspect prerequisites (smoke command; check/demo/prove added in later PRs).
    Doctor(DoctorArgs),
}

// ── doctor ───────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// What to inspect (default: all).
    #[arg(default_value = "all")]
    pub target: DoctorTarget,
}

#[derive(ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum DoctorTarget {
    All,
    Browser,
    Pico,
}
