//! Clean-source entrance for actual reluctant replacement cost/decision observations.
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};
pub(super) fn run(arguments: &[String], dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut output = PathBuf::from("target/reluctant-replan");
    let mut seen = BTreeSet::new();
    let mut args = arguments.iter();
    while let Some(option) = args.next() {
        if !seen.insert(option) {
            return Err(format!("duplicate option {option}").into());
        }
        if option == "--reluctant-replan" {
            continue;
        }
        let value = args.next().ok_or("missing reluctant-replan option value")?;
        match option.as_str() {
            "--output" => output = value.into(),
            _ => return Err(format!("unknown reluctant-replan option {option}").into()),
        }
    }
    let root = crate::workspace::workspace_root()?;
    let output = if output.is_absolute() {
        output
    } else {
        root.join(output)
    };
    if output
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("reluctant-replan output must not contain parent traversal".into());
    }
    if let Ok(relative) = output.strip_prefix(&root) {
        if relative.components().next()
            != Some(std::path::Component::Normal(std::ffi::OsStr::new("target")))
        {
            return Err(
                "reluctant-replan output must be outside tracked source or under target".into(),
            );
        }
    }
    if dry_run {
        println!(
            "reluctant-replan release worker: fixed three-pair windows and 1024-request horizon, {}",
            output.display()
        );
        return Ok(());
    }
    let source = super::git(&root, &["rev-parse", "HEAD"])?;
    if !super::git(&root, &["status", "--porcelain"])?.is_empty() {
        return Err("commit clean source before reluctant-replan measurements".into());
    }
    fs::create_dir_all(output.parent().ok_or("output parent missing")?)?;
    fs::create_dir(&output)?;
    let output = output.canonicalize()?;
    let command = [
        "run",
        "--locked",
        "--release",
        "-p",
        "conduit-std-host",
        "--example",
        "reluctant_replan",
        "--",
    ];
    let status = Command::new("cargo")
        .args(command)
        .arg(&output)
        .current_dir(&root)
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_INCREMENTAL", "0")
        .stdout(Stdio::from(fs::File::create(output.join("stdout.log"))?))
        .stderr(Stdio::from(fs::File::create(output.join("stderr.log"))?))
        .status()?;
    let final_source = super::git(&root, &["rev-parse", "HEAD"])?;
    let custody =
        final_source == source && super::git(&root, &["status", "--porcelain"])?.is_empty();
    fs::write(
        output.join("status.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema":"conduit.reluctant-replan-command@1","git_sha":source,"final_git_sha":final_source,
            "source_custody_verified":custody,"worker_success":status.success(),"worker_exit_code":status.code(),
            "command":command,"output":output,"artifact_sha256":super::manifest(&output)?
        }))?,
    )?;
    if !custody || !status.success() {
        return Err("reluctant-replan capture refused; inspect retained status/stderr".into());
    }
    println!(
        "Retained local reluctant-replan results: {}",
        output.display()
    );
    Ok(())
}
