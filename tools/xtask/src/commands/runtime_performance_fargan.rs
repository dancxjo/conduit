//! Source-pinned FARGAN extension of the runtime-performance proof entrance.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};

pub fn run(arguments: &[String], dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = None;
    let mut output = PathBuf::from("target/fargan-runtime-performance");
    let mut repetitions = 3u32;
    let mut owners = "fresh".to_owned();
    for pair in arguments.chunks(2) {
        let [key, value] = pair else {
            return Err("expected option/value pairs".into());
        };
        match key.as_str() {
            "--fargan-fixture" => fixture = Some(PathBuf::from(value)),
            "--output" => output = value.into(),
            "--repetitions" => repetitions = value.parse()?,
            "--fargan-owners" => owners = value.clone(),
            _ => return Err(format!("unsupported FARGAN option {key}").into()),
        }
    }
    if !(2..=20).contains(&repetitions) {
        return Err("FARGAN repetitions must be 2..=20".into());
    }
    if !matches!(owners.as_str(), "fresh" | "prepared") {
        return Err("FARGAN owners must be fresh or prepared".into());
    }
    let root = crate::workspace::workspace_root()?;
    let fixture = fixture
        .ok_or("explicit private FARGAN fixture directory required")?
        .canonicalize()?;
    let output = if output.is_absolute() {
        output
    } else {
        root.join(output)
    };
    if dry_run {
        println!(
            "release FARGAN worker: fixture={}, output={}, repetitions={repetitions}, owners={owners}",
            fixture.display(),
            output.display()
        );
        return Ok(());
    }
    let sha = super::git(&root, &["rev-parse", "HEAD"])?;
    if !super::git(&root, &["status", "--porcelain"])?.is_empty() {
        return Err(
            "freeze and commit the complete worktree before retaining FARGAN measurements".into(),
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
        "kernel-operation-owners,hosted-catalog-cache",
    ];
    let conformance = Command::new("cargo")
        .arg("test")
        .args(common)
        .args(["--test", "fargan_signal_graph", "--test", "fargan_topology"])
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
    if !conformance.success() {
        return Err("FARGAN conformance failed; measurement refused".into());
    }
    let status = Command::new("cargo")
        .arg("run")
        .args(common)
        .args(["--example", "fargan_performance", "--"])
        .arg(&fixture)
        .arg(&output)
        .arg(repetitions.to_string())
        .arg(&owners)
        .current_dir(&root)
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_INCREMENTAL", "0")
        .stdout(Stdio::from(fs::File::create(output.join("stdout.log"))?))
        .stderr(Stdio::from(fs::File::create(output.join("stderr.log"))?))
        .status()?;
    let final_sha = super::git(&root, &["rev-parse", "HEAD"])?;
    let custody = final_sha == sha && super::git(&root, &["status", "--porcelain"])?.is_empty();
    fs::write(
        output.join("status.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema":"conduit.fargan-runtime-performance-command@1","git_sha":sha,"final_git_sha":final_sha,
            "source_custody_verified":custody,"conformance_success":true,"worker_success":status.success(),
            "worker_exit_code":status.code(),"repetitions":repetitions,"owners":owners,"artifact_sha256":super::manifest(&output)?
        }))?,
    )?;
    if !custody || !status.success() {
        return Err("FARGAN result refused; inspect retained status and stderr".into());
    }
    println!(
        "Retained FARGAN runtime performance results: {}",
        output.display()
    );
    Ok(())
}
