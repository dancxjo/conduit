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
    /// Run a check suite.
    Check(CheckArgs),
    /// Run a demo.
    Demo(DemoArgs),
    /// Run a proof.
    Prove(ProveArgs),
    /// Inspect prerequisites.
    Doctor(DoctorArgs),
}

// ── check ────────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// The suite to run (default: all).
    #[arg(default_value = "all")]
    pub suite: CheckSuite,
}

#[derive(ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum CheckSuite {
    All,
    #[value(name = "kernel-s1")]
    KernelS1,
    #[value(name = "kernel-takeover")]
    KernelTakeover,
    #[value(name = "planning-s2")]
    PlanningS2,
    #[value(name = "form-s3")]
    FormS3,
    #[value(name = "browser-s4")]
    BrowserS4,
    Realm,
    Observatory,
    #[value(name = "std-catalog")]
    StdCatalog,
    Simulation,
}

// ── demo ─────────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct DemoArgs {
    /// The demo to run (default: std).
    #[arg(default_value = "std")]
    pub name: DemoName,
}

#[derive(ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum DemoName {
    Std,
    #[value(name = "triple-local")]
    TripleLocal,
}

// ── prove ─────────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct ProveArgs {
    /// The proof to run.
    pub name: ProveName,
}

#[derive(ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum ProveName {
    #[value(name = "std-browser-s4")]
    StdBrowserS4,
    #[value(name = "std-browser")]
    StdBrowser,
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
