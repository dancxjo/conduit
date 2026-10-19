//! Finite source-bound isolated matrix arithmetic proof entrance.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};
pub fn run(arguments: &[String], dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = arguments
        .iter()
        .filter(|a| a.as_str() != "--matrix-projection")
        .collect();
    let mut output = PathBuf::from("target/matrix-projection-performance");
    let mut iterations = 256u32;
    let mut repetitions = 9u32;
    for pair in arguments.chunks(2) {
        let [key, value] = pair else {
            return Err("expected matrix option/value pairs".into());
        };
        match key.as_str() {
            "--output" => output = PathBuf::from(value.as_str()),
            "--iterations" => iterations = value.parse()?,
            "--repetitions" => repetitions = value.parse()?,
            _ => return Err(format!("unsupported matrix option {key}").into()),
        }
    }
    if !(1..=100000).contains(&iterations) || !(3..=101).contains(&repetitions) {
        return Err("finite matrix measurement bounds refused".into());
    }
    let root = crate::workspace::workspace_root()?;
    let output = if output.is_absolute() {
        output
    } else {
        root.join(output)
    };
    if dry_run {
        println!(
            "matrix release worker: output={}, iterations={iterations}, repetitions={repetitions}",
            output.display()
        );
        return Ok(());
    }
    let source = super::git(&root, &["rev-parse", "HEAD"])?;
    if !super::git(&root, &["status", "--porcelain"])?.is_empty() {
        return Err(
            "commit and freeze complete source before retaining matrix measurements".into(),
        );
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&output)?;
    let output = output.canonicalize()?;
    let common = [
        "--locked",
        "--release",
        "-p",
        "conduit-ai",
        "--features",
        "kernel-operation-owners,hosted-catalog-cache,matrix-projection-reference",
    ];
    let proof = Command::new("cargo")
        .arg("test")
        .args(common)
        .args([
            "--test",
            "fixed_tensor_linear",
            "--test",
            "fixed_numeric_linear_back",
        ])
        .current_dir(&root)
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_INCREMENTAL", "0")
        .stdout(Stdio::from(fs::File::create(
            output.join("conformance.stdout.log"),
        )?))
        .stderr(Stdio::from(fs::File::create(
            output.join("conformance.stderr.log"),
        )?))
        .status()?;
    if !proof.success() {
        return Err("matrix conformance failed; measurement refused".into());
    }
    let worker = Command::new("cargo")
        .arg("run")
        .args(common)
        .args(["--example", "matrix_projection_performance", "--"])
        .arg(&output)
        .arg(iterations.to_string())
        .arg(repetitions.to_string())
        .current_dir(&root)
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_INCREMENTAL", "0")
        .stdout(Stdio::from(fs::File::create(output.join("stdout.log"))?))
        .stderr(Stdio::from(fs::File::create(output.join("stderr.log"))?))
        .status()?;
    let final_source = super::git(&root, &["rev-parse", "HEAD"])?;
    let custody =
        source == final_source && super::git(&root, &["status", "--porcelain"])?.is_empty();
    let manifest = super::manifest(&output)?;
    fs::write(
        output.join("status.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"schema":"conduit.matrix-projection-performance-command@1","git_sha":source,"final_git_sha":final_source,"source_custody_verified":custody,"conformance_success":true,"worker_success":worker.success(),"worker_exit_code":worker.code(),"iterations":iterations,"repetitions":repetitions,"artifact_sha256":manifest}),
        )?,
    )?;
    if !custody || !worker.success() {
        return Err("matrix capture refused; inspect retained status".into());
    }
    println!("Retained matrix arithmetic results: {}", output.display());
    Ok(())
}
