//! One retained release-worker entrance, shared by both xtask dispatchers.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
};
#[path = "../../../../proof/performance/runtime/src/config.rs"]
mod config;

fn git(root: &Path, arguments: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err("source custody probe failed".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub fn run(arguments: &[String], dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    if arguments == ["--help"] {
        println!("cargo xtask prove runtime-performance [--output PATH] [--values N] [--repetitions N] [--decisions N] [--samples N]\nRetains release measurements from committed, clean source. Output must be new and outside tracked source (default target/runtime-performance). Finite defaults: 1000000 values, 5 repetitions, 10000 decisions, 2000 samples.");
        return Ok(());
    }
    let config = config::Config::parse(arguments)?;
    let root = crate::workspace::workspace_root()?;
    let output = if config.output.is_absolute() {
        config.output.clone()
    } else {
        root.join(&config.output)
    };
    if dry_run {
        println!(
            "release runtime-performance worker: {config:?}; output {}",
            output.display()
        );
        return Ok(());
    }
    let sha = git(&root, &["rev-parse", "HEAD"])?;
    if !git(&root, &["status", "--porcelain"])?.is_empty() {
        return Err(
            "freeze and commit the complete worktree before retaining a performance result".into(),
        );
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&output)?;
    let output = output.canonicalize()?;
    let stdout = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("stdout.log"))?;
    let stderr = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("stderr.log"))?;
    let worker_arguments = vec![
        "--output".to_owned(),
        output.display().to_string(),
        "--values".to_owned(),
        config.values.to_string(),
        "--repetitions".to_owned(),
        config.repetitions.to_string(),
        "--decisions".to_owned(),
        config.decisions.to_string(),
        "--samples".to_owned(),
        config.samples.to_string(),
    ];
    eprintln!(
        "Building release worker and retaining output in {}",
        output.display()
    );
    let status = Command::new("cargo")
        .args([
            "run",
            "--locked",
            "--release",
            "-p",
            "conduit-runtime-performance",
            "--",
        ])
        .args(&worker_arguments)
        .current_dir(&root)
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_INCREMENTAL", "0")
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .status()?;
    let final_sha = git(&root, &["rev-parse", "HEAD"])?;
    let clean = git(&root, &["status", "--porcelain"])?.is_empty();
    let custody = final_sha == sha && clean;
    let report = serde_json::json!({
        "schema":"conduit.runtime-performance-command@1", "git_sha":sha,
        "final_git_sha":final_sha, "source_custody_verified":custody,
        "worker_exit_code":status.code(), "worker_success":status.success(),
        "command":["cargo", "run", "--locked", "--release", "-p", "conduit-runtime-performance", "--"],
        "worker_arguments":worker_arguments, "config":config,
        "artifact_sha256":manifest(&output)?,
    });
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("status.json"))?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    if !custody {
        return Err("source changed during measurement; retained result is refused".into());
    }
    if !status.success() {
        return Err(format!("worker refused; inspect {}/stderr.log", output.display()).into());
    }
    println!("Retained runtime performance results: {}", output.display());
    Ok(())
}

fn manifest(directory: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut hashes = std::collections::BTreeMap::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            } else if entry.file_type()?.is_file() {
                let mut file = fs::File::open(entry.path())?;
                let mut digest = Sha256::new();
                let mut buffer = [0; 8192];
                loop {
                    let length = file.read(&mut buffer)?;
                    if length == 0 {
                        break;
                    }
                    digest.update(&buffer[..length]);
                }
                hashes.insert(
                    entry.path().strip_prefix(directory)?.display().to_string(),
                    format!("{:x}", digest.finalize()),
                );
            }
        }
    }
    Ok(serde_json::to_value(hashes)?)
}
