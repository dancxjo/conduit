//! Source-sealed, bounded actual two-Host release-worker entrance.
use super::git;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};

pub(super) fn run(arguments: &[String], dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut output = None;
    let mut repetitions = 3usize;
    let mut rotation = 0usize;
    let mut values = None;
    let mut args = arguments.iter();
    let mut selected = false;
    while let Some(option) = args.next() {
        if option == "--two-host" {
            if selected {
                return Err("duplicate --two-host".into());
            }
            selected = true;
            continue;
        }
        let value = args.next().ok_or("missing two-Host argument value")?;
        match option.as_str() {
            "--output" => output = Some(PathBuf::from(value)),
            "--repetitions" => repetitions = value.parse()?,
            "--rotation" => rotation = value.parse()?,
            "--values" => values = Some(value.parse::<u16>()?),
            _ => return Err(format!("unknown two-Host argument {option}").into()),
        }
    }
    if !selected
        || !(1..=20).contains(&repetitions)
        || rotation > 2
        || values.is_some_and(|n| !(1..=64).contains(&n) || rotation != 0)
    {
        return Err("two-Host capture bounds refused".into());
    }
    let root = crate::workspace::workspace_root()?;
    let output = output.unwrap_or_else(|| root.join("target/two-host-performance"));
    let output = if output.is_absolute() {
        output
    } else {
        root.join(output)
    };
    if output
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("two-Host output must not contain parent traversal".into());
    }
    if let Ok(relative) = output.strip_prefix(&root) {
        if relative.components().next()
            != Some(std::path::Component::Normal(std::ffi::OsStr::new("target")))
        {
            return Err("two-Host output must be outside tracked source or under target".into());
        }
    }
    if dry_run {
        println!(
            "two-Host release worker: {repetitions} repetitions, rotation {rotation}, persistent Values {values:?}, output {}",
            output.display()
        );
        return Ok(());
    }
    let source = git(&root, &["rev-parse", "HEAD"])?;
    if !git(&root, &["status", "--porcelain"])?.is_empty() {
        return Err("commit clean source before retaining a two-Host capture".into());
    }
    fs::create_dir_all(output.parent().ok_or("output parent missing")?)?;
    fs::create_dir(&output)?;
    let output = output.canonicalize()?;
    let feature = "conduit-ai/hosted-catalog-cache";
    let conformance = [
        "test",
        "--locked",
        "--release",
        "-p",
        "conduit",
        "--features",
        feature,
        "--test",
        "two_host_kernel_evidence",
    ];
    let status = Command::new("cargo")
        .args(conformance)
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
    fs::write(
        output.join("conformance.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"source":source,"command":conformance,"exit_code":status.code(),"success":status.success(),"role":"optimized conformance, timings discarded"}),
        )?,
    )?;
    if !status.success() {
        return Err("two-Host conformance refused; no measurement launched".into());
    }
    let build = [
        "build",
        "--locked",
        "--release",
        "-p",
        "conduit",
        "--features",
        feature,
        "--example",
        "two_host_transport_performance",
        "--message-format=json-render-diagnostics",
    ];
    let built = Command::new("cargo")
        .args(build)
        .current_dir(&root)
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_INCREMENTAL", "0")
        .env("CONDUIT_TWO_HOST_PERFORMANCE_SOURCE", &source)
        .output()?;
    fs::write(output.join("build.stdout.jsonl"), &built.stdout)?;
    fs::write(output.join("build.stderr.log"), &built.stderr)?;
    if !built.status.success() {
        return Err("two-Host worker build refused".into());
    }
    let artifacts: Vec<_> = String::from_utf8(built.stdout)?
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|row| {
            row["reason"] == "compiler-artifact"
                && row["target"]["name"] == "two_host_transport_performance"
        })
        .filter_map(|row| row["executable"].as_str().map(PathBuf::from))
        .collect();
    if artifacts.len() != 1 {
        return Err("exact two-Host worker executable missing or ambiguous".into());
    }
    if git(&root, &["rev-parse", "HEAD"])? != source
        || !git(&root, &["status", "--porcelain"])?.is_empty()
    {
        return Err("source changed during two-Host preparation".into());
    }
    let toolchain = Command::new("rustc")
        .args(["--version", "--verbose"])
        .current_dir(&root)
        .output()?;
    if !toolchain.status.success() {
        return Err("two-Host toolchain observation refused".into());
    }
    let environment = serde_json::json!({
        "rustc":String::from_utf8(toolchain.stdout)?,
        "os":std::env::consts::OS,"architecture":std::env::consts::ARCH,
        "available_parallelism":std::thread::available_parallelism()?.get(),
        "clock":"std::time::Instant monotonic wall clock; nanosecond representation, resolution not calibrated",
        "isolation":"local shared machine; exclusive CPU and quiet environment not established"
    });
    let worker = &artifacts[0];
    let worker_sha = format!("{:x}", Sha256::digest(fs::read(worker)?));
    let mut worker_args = vec![
        "--output".to_owned(),
        output.to_str().ok_or("output is not UTF-8")?.to_owned(),
        "--repetitions".to_owned(),
        repetitions.to_string(),
        "--rotation".to_owned(),
        rotation.to_string(),
    ];
    if let Some(values) = values {
        worker_args.extend(["--values".to_owned(), values.to_string()]);
    }
    let status = Command::new(worker)
        .current_dir(&root)
        .args(&worker_args)
        .stdout(Stdio::from(fs::File::create(
            output.join("worker.stdout.log"),
        )?))
        .stderr(Stdio::from(fs::File::create(
            output.join("worker.stderr.log"),
        )?))
        .status()?;
    let custody = git(&root, &["rev-parse", "HEAD"])? == source
        && git(&root, &["status", "--porcelain"])?.is_empty()
        && format!("{:x}", Sha256::digest(fs::read(worker)?)) == worker_sha;
    fs::write(
        output.join("command.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"schema":"conduit.two-host-performance-command@1","source_commit":source,"source_custody_verified":custody,"worker":worker,"worker_sha256":worker_sha,"worker_exit_code":status.code(),"worker_success":status.success(),"conformance_success":true,"build_command":build,"features":feature,"worker_arguments":worker_args,"environment":environment,"role":"capture only; independent timing acceptance required"}),
        )?,
    )?;
    if !status.success() || !custody {
        return Err("two-Host worker or source custody refused".into());
    }
    println!("Two-Host capture retained in {}", output.display());
    Ok(())
}
