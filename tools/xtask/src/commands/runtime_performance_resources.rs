//! Clean-source entrance for actual local resident/copy cost observations.
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};
pub(super) fn run(arguments: &[String], dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut output = PathBuf::from("target/resource-replacement-cost");
    let mut repetitions = 3u16;
    let mut seen = BTreeSet::new();
    let mut args = arguments.iter();
    while let Some(option) = args.next() {
        if !seen.insert(option) {
            return Err(format!("duplicate option {option}").into());
        }
        if option == "--resource-replacement" {
            continue;
        }
        let value = args.next().ok_or("missing resource-cost option value")?;
        match option.as_str() {
            "--output" => output = value.into(),
            "--repetitions" => repetitions = value.parse()?,
            _ => return Err(format!("unknown resource-cost option {option}").into()),
        }
    }
    if !(3..=20).contains(&repetitions) {
        return Err("resource-cost repetitions must be 3..20".into());
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
        return Err("resource-cost output must not contain parent traversal".into());
    }
    if let Ok(relative) = output.strip_prefix(&root) {
        if relative.components().next()
            != Some(std::path::Component::Normal(std::ffi::OsStr::new("target")))
        {
            return Err(
                "resource-cost output must be outside tracked source or under target".into(),
            );
        }
    }
    if dry_run {
        println!(
            "resource-cost release worker: {repetitions} repetitions, {}",
            output.display()
        );
        return Ok(());
    }
    let source = super::git(&root, &["rev-parse", "HEAD"])?;
    if !super::git(&root, &["status", "--porcelain"])?.is_empty() {
        return Err("commit clean source before resource-cost measurements".into());
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
        "resource_replacement_cost",
        "--",
    ];
    let status = Command::new("cargo")
        .args(command)
        .arg(&output)
        .arg(repetitions.to_string())
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
            "schema":"conduit.resource-replacement-cost-command@1","git_sha":source,"final_git_sha":final_source,
            "source_custody_verified":custody,"worker_success":status.success(),"worker_exit_code":status.code(),
            "command":command,"output":output,"repetitions":repetitions,"artifact_sha256":super::manifest(&output)?
        }))?,
    )?;
    if !custody || !status.success() {
        return Err("resource-cost capture refused; inspect retained status/stderr".into());
    }
    println!("Retained local resource-cost results: {}", output.display());
    Ok(())
}
