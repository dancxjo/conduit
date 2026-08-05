use std::{
    fmt,
    path::Path,
    process::{Command, ExitStatus, Stdio},
    time::Instant,
};

use serde::Serialize;

use crate::cli::GlobalOpts;

/// A single read-only probe with stable identity and provenance.
#[derive(Debug, Clone)]
pub struct Step {
    /// Stable identifier used in JSON reports and error messages.
    pub id: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// Executable program.
    pub program: &'static str,
    /// Arguments.
    pub args: &'static [&'static str],
}

impl Step {
    pub const fn new(
        id: &'static str,
        description: &'static str,
        program: &'static str,
        args: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            description,
            program,
            args,
        }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.program, self.args.join(" "))
    }
}

/// Captured result of a read-only prerequisite probe.
#[derive(Debug, Clone, Serialize)]
pub struct ProbeOutcome {
    pub id: String,
    pub description: String,
    pub command_line: String,
    pub skipped: bool,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub launch_error: Option<String>,
    pub elapsed_ms: u128,
}

/// Error returned when a prerequisite command or report cannot be produced.
#[derive(Debug)]
pub struct StepError {
    pub id: String,
    pub command_line: String,
    pub status: Option<ExitStatus>,
    /// Optional human-readable message for non-process errors.
    pub message: String,
}

impl StepError {
    pub fn prereq(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            command_line: String::new(),
            status: None,
            message: message.into(),
        }
    }
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.message.is_empty() && self.command_line.is_empty() {
            write!(f, "step '{}': {}", self.id, self.message)
        } else {
            write!(
                f,
                "step '{}' failed ({}): {}",
                self.id,
                self.status
                    .map(|status| status.to_string())
                    .unwrap_or_else(|| "did not start".into()),
                self.command_line
            )
        }
    }
}

impl std::error::Error for StepError {}

/// Run one read-only probe with captured output.
///
/// Missing programs and nonzero exits are returned as data so a doctor command
/// can report every prerequisite in one pass. `--dry-run` never launches the
/// process and returns a skipped record instead.
pub fn run_probe(step: &Step, working_dir: &Path, opts: &GlobalOpts) -> ProbeOutcome {
    let command_line = format!("{} {}", step.program, step.args.join(" "));

    if !opts.quiet && !opts.json {
        println!("» [{}] {}", step.id, step.description);
        println!("  $ {command_line}");
    }

    if opts.dry_run {
        return ProbeOutcome {
            id: step.id.to_string(),
            description: step.description.to_string(),
            command_line,
            skipped: true,
            success: true,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            launch_error: None,
            elapsed_ms: 0,
        };
    }

    let started = Instant::now();
    match Command::new(step.program)
        .args(step.args)
        .current_dir(working_dir)
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => ProbeOutcome {
            id: step.id.to_string(),
            description: step.description.to_string(),
            command_line,
            skipped: false,
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            launch_error: None,
            elapsed_ms: started.elapsed().as_millis(),
        },
        Err(error) => ProbeOutcome {
            id: step.id.to_string(),
            description: step.description.to_string(),
            command_line,
            skipped: false,
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            launch_error: Some(error.to_string()),
            elapsed_ms: started.elapsed().as_millis(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace() -> PathBuf {
        crate::workspace::workspace_root().unwrap()
    }

    fn dry_opts() -> GlobalOpts {
        GlobalOpts {
            dry_run: true,
            quiet: true,
            ..Default::default()
        }
    }

    #[test]
    fn probe_dry_run_does_not_launch_a_missing_program() {
        let step = Step::new("missing", "missing tool", "__no_such_binary__", &[]);
        let outcome = run_probe(&step, &workspace(), &dry_opts());
        assert!(outcome.skipped);
        assert!(outcome.success);
        assert!(outcome.launch_error.is_none());
    }

    #[test]
    fn probe_reports_a_missing_program_without_panicking() {
        let step = Step::new("missing", "missing tool", "__no_such_binary__", &[]);
        let opts = GlobalOpts {
            quiet: true,
            ..Default::default()
        };
        let outcome = run_probe(&step, &workspace(), &opts);
        assert!(!outcome.skipped);
        assert!(!outcome.success);
        assert!(outcome.launch_error.is_some());
    }

    #[test]
    fn step_display_renders_command_line() {
        let step = Step::new("x", "d", "cargo", &["test", "--lib"]);
        assert_eq!(step.to_string(), "cargo test --lib");
    }
}
