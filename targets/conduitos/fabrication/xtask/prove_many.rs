use std::{
    collections::{BTreeSet, VecDeque},
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use clap::{Args, ValueEnum};
use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{profile::Paths, report::git_head, ConduitosArch, ConduitosError};

const SCHEMA: &str = "conduit.conduitos.prove-many/v1";
const MAXIMUM_PROOFS: usize = 8;
const PREPARED_FILES: &[&str] = &[
    "conduitos",
    "conduitos.iso",
    "build.json",
    "image.json",
    "prepared-proof-image.json",
];

#[derive(Args, Debug, Clone)]
pub(super) struct ProveManyArgs {
    /// Exact x86 proof propositions to execute in one prepared environment.
    #[arg(long = "proof", value_enum, required = true)]
    proofs: Vec<X86Proof>,

    /// Maximum child proofs executing concurrently inside this runner.
    #[arg(long, default_value_t = 4, value_parser = parse_max_parallel)]
    max_parallel: usize,

    /// Bounded root for isolated proof outputs, sockets, logs, and results.
    #[arg(long, default_value = "target/conduitos/prove-many")]
    output_root: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum X86Proof {
    Kernel,
    Xhci,
    Usb,
    Hid,
    Keyboard,
    FrontDoor,
    ProductJourney,
    Rescue,
}

impl X86Proof {
    fn as_str(self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::Xhci => "xhci",
            Self::Usb => "usb",
            Self::Hid => "hid",
            Self::Keyboard => "keyboard",
            Self::FrontDoor => "front-door",
            Self::ProductJourney => "product-journey",
            Self::Rescue => "rescue",
        }
    }

    fn uses_prepared_image(self) -> bool {
        matches!(
            self,
            Self::Xhci | Self::Usb | Self::Hid | Self::Keyboard | Self::Rescue
        )
    }

    fn arguments(self, evidence_root: &Path) -> Vec<String> {
        let mut arguments = vec!["conduitos".to_owned()];
        match self {
            Self::Kernel => arguments.extend([
                "prove".to_owned(),
                "--arch".to_owned(),
                "x86-64".to_owned(),
                "--evidence-root".to_owned(),
                evidence_root.display().to_string(),
            ]),
            Self::Xhci => arguments.extend(["xhci-proof", "--prepared-image"].map(str::to_owned)),
            Self::Usb => arguments.extend(["usb-proof", "--prepared-image"].map(str::to_owned)),
            Self::Hid => arguments.extend(["hid-proof", "--prepared-image"].map(str::to_owned)),
            Self::Keyboard => {
                arguments.extend(["keyboard-proof", "--prepared-image"].map(str::to_owned));
            }
            Self::FrontDoor => arguments.push("front-door-proof".to_owned()),
            Self::ProductJourney => arguments.push("journey-proof".to_owned()),
            Self::Rescue => {
                arguments.extend(["rescue-proof", "--prepared-image"].map(str::to_owned));
            }
        }
        arguments.push("--locked".to_owned());
        arguments
    }
}

#[derive(Serialize)]
struct ProofResult {
    schema: &'static str,
    proof: X86Proof,
    status: &'static str,
    command: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verification_command: Option<Vec<String>>,
    isolated_target_root: String,
    stdout_log: String,
    stderr_log: String,
    started_order: usize,
    finished_order: usize,
}

#[derive(Serialize)]
struct BatchResult<'a> {
    schema: &'static str,
    maximum_parallel: usize,
    maximum_observed_parallelism: usize,
    results: &'a [ProofResult],
}

struct RunningProof {
    proof: X86Proof,
    child: Child,
    command: Vec<String>,
    target_root: PathBuf,
    stdout_log: PathBuf,
    stderr_log: PathBuf,
    started_order: usize,
}

fn parse_max_parallel(value: &str) -> Result<usize, String> {
    let value: usize = value
        .parse()
        .map_err(|_| "max-parallel must be an integer".to_owned())?;
    if (1..=MAXIMUM_PROOFS).contains(&value) {
        Ok(value)
    } else {
        Err(format!("max-parallel must be 1..={MAXIMUM_PROOFS}"))
    }
}

pub(super) fn execute(args: ProveManyArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-proof-batch",
            "prove-many must retain exact results from executed proofs",
        ));
    }
    let proofs = validated_proofs(&args.proofs)?;
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let output_root = absolute_output_root(&paths.root, &args.output_root)?;
    fs::create_dir_all(&output_root).map_err(|error| {
        ConduitosError::refusal("proof-batch-root-unavailable", error.to_string())
    })?;
    refuse_nonempty_root(&output_root)?;
    let shared_cargo_target = paths.root.join("target");
    let commit = git_head(&paths.root)?;
    let executable = std::env::current_exe().map_err(|error| {
        ConduitosError::refusal("proof-batch-executable-unavailable", error.to_string())
    })?;

    let mut pending = VecDeque::from(proofs);
    let mut running = Vec::new();
    let mut results = Vec::new();
    let mut started = 0;
    let mut finished = 0;
    let mut maximum_observed_parallelism = 0;

    while !pending.is_empty() || !running.is_empty() {
        while running.len() < args.max_parallel {
            let Some(proof) = pending.pop_front() else {
                break;
            };
            started += 1;
            let spawned = spawn_proof(
                proof,
                started,
                &executable,
                &paths,
                &output_root,
                &shared_cargo_target,
            );
            match spawned {
                Ok(proof) => running.push(proof),
                Err(error) => {
                    stop_running(&mut running);
                    return Err(error);
                }
            }
            maximum_observed_parallelism = maximum_observed_parallelism.max(running.len());
        }

        let mut completed = None;
        let mut polling_error = None;
        for (index, proof) in running.iter_mut().enumerate() {
            match proof.child.try_wait() {
                Ok(Some(status)) => {
                    completed = Some((index, status.success()));
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    polling_error = Some(ConduitosError::refusal(
                        "proof-batch-child-unavailable",
                        format!("{}: {error}", proof.proof.as_str()),
                    ));
                    break;
                }
            }
        }
        if let Some(error) = polling_error {
            stop_running(&mut running);
            return Err(error);
        }
        if let Some((index, success)) = completed {
            let proof = running.remove(index);
            let (success, verification_command) = if success && proof.proof == X86Proof::Kernel {
                let command = kernel_verification_arguments(&proof.target_root, &commit);
                match run_verifier(&executable, &command, &proof) {
                    Ok(success) => (success, Some(command)),
                    Err(error) => {
                        stop_running(&mut running);
                        return Err(error);
                    }
                }
            } else {
                (success, None)
            };
            finished += 1;
            let result = ProofResult {
                schema: "conduit.conduitos.prove-many-result/v1",
                proof: proof.proof,
                status: if success { "success" } else { "failure" },
                command: proof.command,
                verification_command,
                isolated_target_root: proof.target_root.display().to_string(),
                stdout_log: proof.stdout_log.display().to_string(),
                stderr_log: proof.stderr_log.display().to_string(),
                started_order: proof.started_order,
                finished_order: finished,
            };
            write_json(
                &output_root
                    .join("results")
                    .join(format!("{}.json", proof.proof.as_str())),
                &result,
            )?;
            results.push(result);
        } else {
            thread::sleep(Duration::from_millis(50));
        }
    }

    results.sort_by_key(|result| result.proof.as_str());
    write_json(
        &output_root.join("summary.json"),
        &BatchResult {
            schema: SCHEMA,
            maximum_parallel: args.max_parallel,
            maximum_observed_parallelism,
            results: &results,
        },
    )?;
    let failures = failure_names(&results);
    if !failures.is_empty() {
        return Err(ConduitosError::refusal(
            "proof-batch-failed",
            format!("failed proofs: {}", failures.join(", ")),
        ));
    }
    if !opts.quiet && !opts.json {
        println!("ConduitOS x86 proof batch: {}", output_root.display());
    }
    Ok(())
}

fn failure_names(results: &[ProofResult]) -> Vec<&'static str> {
    results
        .iter()
        .filter(|result| result.status != "success")
        .map(|result| result.proof.as_str())
        .collect()
}

fn refuse_nonempty_root(output_root: &Path) -> Result<(), ConduitosError> {
    let mut entries = fs::read_dir(output_root).map_err(|error| {
        ConduitosError::refusal("proof-batch-root-unavailable", error.to_string())
    })?;
    if entries
        .next()
        .transpose()
        .map_err(|error| {
            ConduitosError::refusal("proof-batch-root-unavailable", error.to_string())
        })?
        .is_some()
    {
        return Err(ConduitosError::refusal(
            "proof-batch-root-not-empty",
            format!(
                "refusing to mix new proof with existing bytes in {}",
                output_root.display()
            ),
        ));
    }
    Ok(())
}

fn stop_running(running: &mut [RunningProof]) {
    for proof in running.iter_mut() {
        let _ = proof.child.kill();
    }
    for proof in running.iter_mut() {
        let _ = proof.child.wait();
    }
}

fn kernel_verification_arguments(target_root: &Path, commit: &str) -> Vec<String> {
    vec![
        "evidence".to_owned(),
        "verify".to_owned(),
        "--root".to_owned(),
        target_root
            .parent()
            .expect("run root")
            .join("evidence")
            .display()
            .to_string(),
        "--commit".to_owned(),
        commit.to_owned(),
        "--result".to_owned(),
        "complete".to_owned(),
        "--proof".to_owned(),
        "conduitos-x86_64".to_owned(),
        "--suite".to_owned(),
        "conduitos.prove.x86_64".to_owned(),
        "--locked".to_owned(),
    ]
}

fn run_verifier(
    executable: &Path,
    arguments: &[String],
    proof: &RunningProof,
) -> Result<bool, ConduitosError> {
    let stdout = OpenOptions::new()
        .append(true)
        .open(&proof.stdout_log)
        .map_err(|error| {
            ConduitosError::refusal("proof-batch-log-unavailable", error.to_string())
        })?;
    let stderr = OpenOptions::new()
        .append(true)
        .open(&proof.stderr_log)
        .map_err(|error| {
            ConduitosError::refusal("proof-batch-log-unavailable", error.to_string())
        })?;
    let status = Command::new(executable)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .status()
        .map_err(|error| {
            ConduitosError::refusal("proof-batch-verifier-unavailable", error.to_string())
        })?;
    Ok(status.success())
}

fn validated_proofs(proofs: &[X86Proof]) -> Result<Vec<X86Proof>, ConduitosError> {
    if proofs.is_empty() || proofs.len() > MAXIMUM_PROOFS {
        return Err(ConduitosError::refusal(
            "proof-batch-cardinality-invalid",
            format!("expected 1..={MAXIMUM_PROOFS}, got {}", proofs.len()),
        ));
    }
    let unique: BTreeSet<_> = proofs.iter().copied().collect();
    if unique.len() != proofs.len() {
        return Err(ConduitosError::refusal(
            "proof-batch-duplicate",
            "each exact proof may appear once",
        ));
    }
    Ok(proofs.to_vec())
}

fn absolute_output_root(root: &Path, requested: &Path) -> Result<PathBuf, ConduitosError> {
    if requested
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(ConduitosError::refusal(
            "proof-batch-root-invalid",
            "output root may not contain parent traversal",
        ));
    }
    Ok(if requested.is_absolute() {
        requested.to_owned()
    } else {
        root.join(requested)
    })
}

fn spawn_proof(
    proof: X86Proof,
    started_order: usize,
    executable: &Path,
    prepared_paths: &Paths,
    output_root: &Path,
    shared_cargo_target: &Path,
) -> Result<RunningProof, ConduitosError> {
    let run_root = output_root.join("runs").join(proof.as_str());
    let target_root = run_root.join("conduitos");
    let logs = run_root.join("logs");
    fs::create_dir_all(&logs).map_err(|error| {
        ConduitosError::refusal("proof-batch-run-root-unavailable", error.to_string())
    })?;
    if proof.uses_prepared_image() {
        copy_prepared_image(&prepared_paths.target, &target_root.join("x86_64"))?;
    }
    let stdout_log = logs.join("stdout.log");
    let stderr_log = logs.join("stderr.log");
    let evidence_root = run_root.join("evidence");
    let arguments = proof.arguments(&evidence_root);
    let child = Command::new(executable)
        .args(&arguments)
        .env("CONDUIT_CONDUITOS_TARGET_ROOT", &target_root)
        .env("CARGO_TARGET_DIR", shared_cargo_target)
        .stdin(Stdio::null())
        .stdout(Stdio::from(File::create(&stdout_log).map_err(|error| {
            ConduitosError::refusal("proof-batch-log-unavailable", error.to_string())
        })?))
        .stderr(Stdio::from(File::create(&stderr_log).map_err(|error| {
            ConduitosError::refusal("proof-batch-log-unavailable", error.to_string())
        })?))
        .spawn()
        .map_err(|error| {
            ConduitosError::refusal(
                "proof-batch-child-unavailable",
                format!("{}: {error}", proof.as_str()),
            )
        })?;
    Ok(RunningProof {
        proof,
        child,
        command: arguments,
        target_root,
        stdout_log,
        stderr_log,
        started_order,
    })
}

fn copy_prepared_image(source: &Path, destination: &Path) -> Result<(), ConduitosError> {
    fs::create_dir_all(destination).map_err(|error| {
        ConduitosError::refusal("proof-batch-image-copy-failed", error.to_string())
    })?;
    for name in PREPARED_FILES {
        let source_file = source.join(name);
        let destination_file = destination.join(name);
        if fs::hard_link(&source_file, &destination_file).is_err() {
            fs::copy(&source_file, &destination_file).map_err(|error| {
                ConduitosError::refusal("proof-batch-image-copy-failed", format!("{name}: {error}"))
            })?;
        }
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), ConduitosError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ConduitosError::refusal("proof-batch-result-failed", error.to_string())
        })?;
    }
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| ConduitosError::refusal("proof-batch-result-failed", error.to_string()))?;
    bytes.push(b'\n');
    fs::write(path, bytes)
        .map_err(|error| ConduitosError::refusal("proof-batch-result-failed", error.to_string()))
}

#[cfg(test)]
mod tests;
