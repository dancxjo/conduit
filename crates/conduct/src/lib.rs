//! Shared command model and presentation-neutral CLI policy.

use std::path::PathBuf;

pub mod run_stream;

use clap::{
    ArgAction, ArgGroup, Args as ClapArgs, Command, CommandFactory, Parser, Subcommand, ValueEnum,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Check,
    Explain,
    Run,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum DiagnosticFormat {
    #[default]
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

/// Primary stdout encoding, deliberately independent of diagnostic encoding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Ndjson,
}

/// Artifact marker requested for read-only inspection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum InspectKind {
    #[default]
    Auto,
    Panel,
    LoweredSource,
    ExecutionPlan,
    Evidence,
    Diagnostic,
    Conformance,
    Package,
}

/// Additive operations that do not reinterpret ordinary paths after `--`.
#[derive(Debug, Eq, PartialEq, Subcommand)]
pub enum SecondaryCommand {
    /// Validate and describe one artifact without executing it.
    Inspect(InspectArguments),
    /// Compile source against explicit immutable inputs into one exact plan.
    Compile(CompileArguments),
    /// Create, verify, or extract a bounded content-addressed package.
    Package(PackageArguments),
    /// Pack, inspect, check, unpack, or diff an authored panel capsule.
    Capsule(CapsuleArguments),
}

#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct CapsuleArguments {
    #[command(subcommand)]
    pub operation: CapsuleOperation,
}

#[derive(Debug, Eq, PartialEq, Subcommand)]
pub enum CapsuleOperation {
    /// Create one canonical capsule JSON document without fetching artifacts.
    Pack(CapsulePackArguments),
    /// Validate and describe a capsule without executing its source.
    Inspect(CapsulePathArguments),
    /// Validate the capsule and parse its authored panel offline.
    Check(CapsulePathArguments),
    /// Resolve and explain the capsule source without fetching artifacts.
    Explain(CapsulePathArguments),
    /// Write source and optional auxiliary documents to a new directory.
    Unpack(CapsuleUnpackArguments),
    /// Compare authored, lock, reference, and presentation identities.
    Diff(CapsuleDiffArguments),
}

#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct CapsulePackArguments {
    #[arg(value_name = "PANEL")]
    pub panel: PathBuf,
    #[arg(long, value_name = "LOCK")]
    pub lock: Option<PathBuf>,
    #[arg(long, value_name = "PRESENTATION")]
    pub presentation: Option<PathBuf>,
    #[arg(long, value_name = "REFERENCES")]
    pub references: Option<PathBuf>,
    #[arg(long, value_name = "CAPSULE")]
    pub output: PathBuf,
}

#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct CapsulePathArguments {
    #[arg(value_name = "CAPSULE")]
    pub capsule: PathBuf,
}

#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct CapsuleUnpackArguments {
    #[arg(value_name = "CAPSULE")]
    pub capsule: PathBuf,
    #[arg(long, value_name = "DIRECTORY")]
    pub output_dir: PathBuf,
}

#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct CapsuleDiffArguments {
    #[arg(value_name = "LEFT")]
    pub left: PathBuf,
    #[arg(value_name = "RIGHT")]
    pub right: PathBuf,
}

/// Inputs for bounded, non-executing artifact inspection.
#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct InspectArguments {
    /// Select a current artifact kind, or use marker-only detection.
    #[arg(long = "type", value_name = "TYPE", value_enum, default_value_t)]
    pub kind: InspectKind,

    /// Read the artifact from this file, or `-` for stdin.
    #[arg(value_name = "ARTIFACT")]
    pub artifact: PathBuf,
}

/// Explicit exact-plan compilation inputs.
#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct CompileArguments {
    /// Read the sealed compile-input document from this JSON file.
    #[arg(long, value_name = "INPUT")]
    pub input: PathBuf,

    /// Read editable source from this file, or `-` for stdin.
    #[arg(value_name = "PANEL")]
    pub panel: PathBuf,
}

/// Bounded package workflow.
#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct PackageArguments {
    #[command(subcommand)]
    pub operation: PackageOperation,
}

#[derive(Debug, Eq, PartialEq, Subcommand)]
pub enum PackageOperation {
    /// Create one deterministic thick or thin package.
    Create(PackageCreateArguments),
    /// Validate package metadata against explicit trust observations.
    Verify(PackageVerifyArguments),
    /// Validate and extract embedded blobs to digest-derived paths.
    Extract(PackageExtractArguments),
}

#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct PackageCreateArguments {
    /// Read the sealed package manifest from this JSON file.
    #[arg(long, value_name = "MANIFEST")]
    pub manifest: PathBuf,

    /// Add one exact embedded blob as SHA256=PATH; repeat as needed.
    #[arg(long = "blob", value_name = "SHA256=PATH")]
    pub blobs: Vec<String>,

    /// Write the deterministic package envelope to this new path.
    #[arg(long, value_name = "PACKAGE")]
    pub output: PathBuf,
}

#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct PackageExtractArguments {
    /// Read and validate this package envelope.
    #[arg(value_name = "PACKAGE")]
    pub package: PathBuf,

    /// Create digest-derived blob paths beneath this directory.
    #[arg(long, value_name = "DIRECTORY")]
    pub output_dir: PathBuf,
}

#[derive(Debug, Eq, PartialEq, ClapArgs)]
pub struct PackageVerifyArguments {
    /// Read and validate this package envelope.
    #[arg(value_name = "PACKAGE")]
    pub package: PathBuf,

    /// Read the explicit package trust policy from this JSON file.
    #[arg(long, value_name = "POLICY")]
    pub policy: PathBuf,

    /// Read external signature verification observations from this JSON file.
    #[arg(long, value_name = "OBSERVATIONS")]
    pub observations: PathBuf,
}

/// Check, explain, and run one typed node arrangement.
#[derive(Debug, Eq, Parser, PartialEq)]
#[command(
    name = "conduct",
    version,
    about = "Conduct a typed node arrangement.",
    override_usage = "conduct [--check | --explain | --run] [PANEL | -]",
    disable_help_subcommand = true,
    group(ArgGroup::new("mode").args(["check", "explain", "run"]).multiple(false))
)]
pub struct Arguments {
    /// Parse, resolve, and validate without starting nodes.
    #[arg(long, group = "mode")]
    pub check: bool,

    /// Show exact node, port, cord, type, and flow resolution.
    #[arg(long, group = "mode")]
    pub explain: bool,

    /// Run the panel (the default mode).
    #[arg(long, group = "mode")]
    pub run: bool,

    /// Select human, finite JSON, or streaming NDJSON primary output.
    #[arg(long, value_enum, default_value_t, global = true)]
    pub format: OutputFormat,

    /// Select human or lossless JSON diagnostics on stderr.
    #[arg(long, value_enum, default_value_t, global = true)]
    pub diagnostic_format: DiagnosticFormat,

    /// Select diagnostic terminal styling.
    #[arg(long, value_enum, default_value_t, global = true)]
    pub color: ColorChoice,

    /// Suppress nonessential status and progress, never values or diagnostics.
    #[arg(short, long, conflicts_with = "verbose", global = true)]
    pub quiet: bool,

    /// Add bounded resolution status detail; repeat for future detail levels.
    #[arg(
        short = 'v',
        action = ArgAction::Count,
        conflicts_with = "quiet",
        global = true
    )]
    pub verbose: u8,

    /// Include related spans, notes, paths, and causes.
    #[arg(long, global = true)]
    pub verbose_diagnostics: bool,

    /// Read editable source from this file, or `-`/no path for stdin.
    #[arg(value_name = "PANEL")]
    pub panel: Option<PathBuf>,

    /// Resolve and run against this explicit compile-input snapshot.
    #[arg(long, value_name = "INPUT")]
    pub compile_input: Option<PathBuf>,

    /// Run the finite batch compatibility demo instead of an exact plan.
    #[arg(long, conflicts_with = "compile_input")]
    pub compatibility_demo: bool,

    /// Explicitly install the bounded example file-write provider.
    #[arg(long)]
    pub enable_file_write: bool,

    /// Explicitly install the bounded example file-watch provider.
    #[arg(long)]
    pub enable_file_watch: bool,

    /// Explicitly install the bounded evictable blob-cache provider.
    #[arg(long)]
    pub enable_storage_cache: bool,

    /// Explicitly install the bounded closed-inventory process provider.
    #[arg(long)]
    pub enable_process_exec: bool,

    /// Explicitly install the bounded numeric-loopback socket providers.
    #[arg(long)]
    pub enable_socket_loopback: bool,

    /// Explicitly install the bounded numeric-loopback HTTP client provider.
    #[arg(long)]
    pub enable_http_client_loopback: bool,

    /// Observe and explicitly install the closed-inventory hosted ALSA audio provider.
    #[arg(long)]
    pub enable_audio_alsa: bool,

    /// Additive read-only operations.
    #[command(subcommand)]
    pub secondary: Option<SecondaryCommand>,
}

impl Arguments {
    #[must_use]
    pub const fn mode(&self) -> Mode {
        if self.check {
            Mode::Check
        } else if self.explain {
            Mode::Explain
        } else {
            Mode::Run
        }
    }
}

/// Builds the sole command model used by parsing, help, completions, and the
/// manual page.
#[must_use]
pub fn command() -> Command {
    Arguments::command()
}

/// Exact bounded-progress state. It has no renderer and cannot exceed its
/// declared total.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundedProgress {
    current: u64,
    total: u64,
    cancelled: bool,
}

impl BoundedProgress {
    pub fn new(total: u64) -> Result<Self, ProgressError> {
        if total == 0 {
            return Err(ProgressError::ZeroTotal);
        }
        Ok(Self {
            current: 0,
            total,
            cancelled: false,
        })
    }

    pub fn advance_to(&mut self, current: u64) -> Result<(), ProgressError> {
        if current < self.current || current > self.total {
            return Err(ProgressError::OutOfBounds);
        }
        self.current = current;
        Ok(())
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    #[must_use]
    pub const fn current(self) -> u64 {
        self.current
    }

    #[must_use]
    pub const fn total(self) -> u64 {
        self.total
    }

    #[must_use]
    pub const fn is_cancelled(self) -> bool {
        self.cancelled
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressError {
    ZeroTotal,
    OutOfBounds,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_model_defaults_to_run_and_keeps_formats_distinct() {
        let arguments = Arguments::try_parse_from([
            "conduct",
            "--format=json",
            "--diagnostic-format=human",
            "panel.panel",
        ])
        .unwrap();
        assert_eq!(arguments.mode(), Mode::Run);
        assert_eq!(arguments.format, OutputFormat::Json);
        assert_eq!(arguments.diagnostic_format, DiagnosticFormat::Human);
    }

    #[test]
    fn bounded_progress_rejects_unknown_and_reversing_work() {
        assert_eq!(BoundedProgress::new(0), Err(ProgressError::ZeroTotal));
        let mut progress = BoundedProgress::new(3).unwrap();
        progress.advance_to(2).unwrap();
        assert_eq!(progress.advance_to(1), Err(ProgressError::OutOfBounds));
        assert_eq!(progress.advance_to(4), Err(ProgressError::OutOfBounds));
        progress.cancel();
        assert!(progress.is_cancelled());
    }

    #[test]
    fn general_and_diagnostic_verbosity_are_independent() {
        let arguments =
            Arguments::try_parse_from(["conduct", "-vv", "--verbose-diagnostics", "--check"])
                .unwrap();
        assert_eq!(arguments.verbose, 2);
        assert!(arguments.verbose_diagnostics);

        let error = Arguments::try_parse_from(["conduct", "--quiet", "-v"]).unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn every_progress_conformance_vector_is_enforced() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../conformance/c3/conduct-output.json"))
                .unwrap();
        for case in fixture["progress_cases"].as_array().unwrap() {
            let total = case["total"].as_u64().unwrap();
            let expected = case["expected"]["accepted"].as_bool().unwrap();
            let result = BoundedProgress::new(total).and_then(|mut progress| {
                for update in case
                    .get("updates")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    progress.advance_to(update.as_u64().unwrap())?;
                }
                Ok(progress)
            });
            assert_eq!(result.is_ok(), expected, "{}", case["id"]);
        }
    }
}
