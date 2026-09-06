use std::{
    env,
    ffi::OsStr,
    fmt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::Instant,
};

use serde::Serialize;

use crate::{cli::GlobalOpts, proof::ProofClass};

/// A single read-only probe or orchestrated task step with stable identity and provenance.
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
    /// Working directory relative to workspace root (None for root).
    pub cwd: Option<&'static str>,
    /// Required tool or build target.
    pub tool_or_target: Option<&'static str>,
    /// Proof class or proof purpose.
    pub proof_class: Option<ProofClass>,
    /// Expected output artifacts.
    pub expected_artifacts: &'static [&'static str],
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
            cwd: None,
            tool_or_target: None,
            proof_class: None,
            expected_artifacts: &[],
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub const fn typed(
        id: &'static str,
        description: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        cwd: Option<&'static str>,
        tool_or_target: Option<&'static str>,
        proof_class: Option<ProofClass>,
        expected_artifacts: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            description,
            program,
            args,
            cwd,
            tool_or_target,
            proof_class,
            expected_artifacts,
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
    match command_for(step.program)
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

/// Run a sequence of orchestrated steps in order.
pub fn run_suite(steps: &[Step], root: &Path, opts: &GlobalOpts) -> Result<(), StepError> {
    run_suite_with_environment(steps, root, opts, &[])
}

/// Run a suite with a finite set of explicit environment values inherited by each step.
pub fn run_suite_with_environment(
    steps: &[Step],
    root: &Path,
    opts: &GlobalOpts,
    environment: &[(&str, &OsStr)],
) -> Result<(), StepError> {
    for step in steps {
        run_step_with_environment(step, root, opts, environment)?;
    }
    Ok(())
}

/// Run one orchestrated step, forwarding `--locked` to Cargo when specified in `opts`.
pub fn run_step(step: &Step, root: &Path, opts: &GlobalOpts) -> Result<(), StepError> {
    run_step_with_environment(step, root, opts, &[])
}

/// Run one orchestrated step with an exact dynamically planned argument list.
pub fn run_step_with_arguments(
    step: &Step,
    arguments: &[String],
    root: &Path,
    opts: &GlobalOpts,
) -> Result<(), StepError> {
    run_step_with_arguments_and_environment(step, arguments.to_vec(), root, opts, &[])
}

fn run_step_with_environment(
    step: &Step,
    root: &Path,
    opts: &GlobalOpts,
    environment: &[(&str, &OsStr)],
) -> Result<(), StepError> {
    let arguments = step
        .args
        .iter()
        .map(|argument| argument.to_string())
        .collect();
    run_step_with_arguments_and_environment(step, arguments, root, opts, environment)
}

fn run_step_with_arguments_and_environment(
    step: &Step,
    mut effective_args: Vec<String>,
    root: &Path,
    opts: &GlobalOpts,
    environment: &[(&str, &OsStr)],
) -> Result<(), StepError> {
    let work_dir = match step.cwd {
        Some(rel) => root.join(rel),
        None => root.to_path_buf(),
    };

    if opts.locked
        && step.program == "cargo"
        && !effective_args.is_empty()
        && effective_args[0] != "fmt"
        && !effective_args.iter().any(|a| a == "--locked")
    {
        effective_args.insert(1, "--locked".to_string());
    }

    let command_line = format!("{} {}", step.program, effective_args.join(" "));

    if !opts.quiet && !opts.json {
        let meta = match (step.tool_or_target, step.proof_class) {
            (Some(t), Some(p)) => format!(" {t}/{}", p.as_str()),
            (Some(t), None) => format!(" {t}"),
            (None, Some(p)) => format!(" {}", p.as_str()),
            (None, None) => String::new(),
        };
        println!("» [{}{}] {}", step.id, meta, step.description);
        println!("  $ {command_line}");
        if !step.expected_artifacts.is_empty() {
            println!("    artifacts: {}", step.expected_artifacts.join(", "));
        }
    }

    if opts.dry_run {
        return Ok(());
    }

    let mut cmd = command_for(step.program);
    cmd.args(&effective_args)
        .current_dir(&work_dir)
        .envs(environment.iter().copied());

    match cmd.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(StepError {
            id: step.id.to_string(),
            command_line,
            status: Some(status),
            message: String::new(),
        }),
        Err(error) => Err(StepError {
            id: step.id.to_string(),
            command_line,
            status: None,
            message: error.to_string(),
        }),
    }
}

/// Build a command for a tool, accepting Cargo-installed binaries even when the
/// invoking shell did not source Cargo's PATH setup.
pub fn command_for(program: &str) -> Command {
    Command::new(resolve_program(program))
}

fn resolve_program(program: &str) -> PathBuf {
    let program_path = Path::new(program);
    if program_path.components().count() > 1 || path_contains_program(program) {
        return program_path.to_path_buf();
    }

    if let Some(path) = cargo_bin_program(program).filter(|path| path.is_file()) {
        return path;
    }

    program_path.to_path_buf()
}

fn path_contains_program(program: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|paths| env::split_paths(&paths).any(|path| path.join(program).is_file()))
}

fn cargo_bin_program(program: &str) -> Option<PathBuf> {
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))?;
    Some(cargo_home.join("bin").join(program))
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

    #[test]
    fn run_step_dry_run_succeeds() {
        let step = Step::typed(
            "test.step",
            "test step",
            "cargo",
            &["check", "-p", "conduit-kernel"],
            None,
            Some("kernel"),
            Some(ProofClass::DeterministicUnit),
            &[],
        );
        let result = run_step(&step, &workspace(), &dry_opts());
        assert!(result.is_ok());
    }

    #[test]
    fn run_suite_dry_run_executes_all_steps() {
        let steps = &[
            Step::new("step1", "step one", "cargo", &["fmt", "--check"]),
            Step::new("step2", "step two", "cargo", &["check"]),
        ];
        let result = run_suite(steps, &workspace(), &dry_opts());
        assert!(result.is_ok());
    }
}
