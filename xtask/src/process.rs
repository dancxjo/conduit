use std::{
    fmt,
    path::Path,
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

use crate::cli::GlobalOpts;

/// A single suite step with identity and provenance.
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
        Self { id, description, program, args }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.program, self.args.join(" "))
    }
}

/// Outcome of a single executed step.
#[derive(Debug)]
pub struct StepOutcome {
    pub id: String,
    pub description: String,
    pub command_line: String,
    pub elapsed: Duration,
    pub status: Option<ExitStatus>,
    pub skipped: bool,
}

impl StepOutcome {
    pub fn success(&self) -> bool {
        self.skipped || self.status.map(|s| s.success()).unwrap_or(false)
    }
}

/// Error returned when a step fails or a prerequisite check fails.
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
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "did not start".into()),
                self.command_line
            )
        }
    }
}

impl std::error::Error for StepError {}

/// Run a sequence of steps, respecting global options.
///
/// Returns `Ok(outcomes)` when all steps pass, or the first `Err` when a step
/// fails and `--keep-going` is not set. With `--keep-going` all steps run and
/// the last error is returned if any failed.
pub fn run_steps(
    steps: &[Step],
    working_dir: &Path,
    opts: &GlobalOpts,
) -> Result<Vec<StepOutcome>, StepError> {
    let mut outcomes: Vec<StepOutcome> = Vec::new();
    let mut last_err: Option<StepError> = None;

    for step in steps {
        let command_line = format!("{} {}", step.program, step.args.join(" "));

        if !opts.quiet {
            println!("» [{id}] {desc}", id = step.id, desc = step.description);
            println!("  $ {command_line}");
        }

        if opts.dry_run {
            outcomes.push(StepOutcome {
                id: step.id.to_string(),
                description: step.description.to_string(),
                command_line,
                elapsed: Duration::ZERO,
                status: None,
                skipped: true,
            });
            continue;
        }

        let start = Instant::now();
        let status = Command::new(step.program)
            .args(step.args)
            .current_dir(working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();
        let elapsed = start.elapsed();

        let outcome = match status {
            Ok(s) => StepOutcome {
                id: step.id.to_string(),
                description: step.description.to_string(),
                command_line: command_line.clone(),
                elapsed,
                status: Some(s),
                skipped: false,
            },
            Err(e) => {
                eprintln!("failed to launch '{}': {e}", step.program);
                StepOutcome {
                    id: step.id.to_string(),
                    description: step.description.to_string(),
                    command_line: command_line.clone(),
                    elapsed,
                    status: None,
                    skipped: false,
                }
            }
        };

        let success = outcome.success();
        outcomes.push(outcome);

        if !success {
            let err = StepError {
                id: step.id.to_string(),
                command_line,
                status: outcomes.last().and_then(|o| o.status),
                message: String::new(),
            };
            if opts.keep_going {
                last_err = Some(err);
            } else {
                return Err(err);
            }
        }
    }

    if let Some(err) = last_err {
        Err(err)
    } else {
        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::GlobalOpts;
    use std::path::PathBuf;

    fn workspace() -> PathBuf {
        crate::workspace::workspace_root().unwrap()
    }

    fn dry_opts() -> GlobalOpts {
        GlobalOpts { dry_run: true, ..Default::default() }
    }

    #[test]
    fn dry_run_skips_all_steps() {
        let steps = [
            Step::new("a", "desc a", "cargo", &["--version"]),
            Step::new("b", "desc b", "cargo", &["--version"]),
        ];
        let outcomes = run_steps(&steps, &workspace(), &dry_opts()).unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| o.skipped));
    }

    #[test]
    fn dry_run_produces_correct_ids() {
        let steps = [Step::new("my-step", "desc", "cargo", &["test"])];
        let outcomes = run_steps(&steps, &workspace(), &dry_opts()).unwrap();
        assert_eq!(outcomes[0].id, "my-step");
    }

    #[test]
    fn step_display_renders_command_line() {
        let step = Step::new("x", "d", "cargo", &["test", "--lib"]);
        assert_eq!(step.to_string(), "cargo test --lib");
    }

    #[test]
    fn keep_going_runs_all_steps_and_returns_err() {
        // Use a program that does not exist to force failure on both steps.
        let steps = [
            Step::new("fail-a", "d", "__no_such_binary__", &[]),
            Step::new("fail-b", "d", "__no_such_binary__", &[]),
        ];
        let opts = GlobalOpts { keep_going: true, quiet: true, ..Default::default() };
        let result = run_steps(&steps, &workspace(), &opts);
        // Both steps attempted; an error is returned.
        assert!(result.is_err());
    }

    #[test]
    fn first_failure_stops_early() {
        let steps = [
            Step::new("fail", "d", "__no_such_binary__", &[]),
            Step::new("ok", "d", "cargo", &["--version"]),
        ];
        let opts = GlobalOpts { quiet: true, ..Default::default() };
        let result = run_steps(&steps, &workspace(), &opts);
        assert!(result.is_err());
    }
}
