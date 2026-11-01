//! Independent retained capture replay; never a timing or release admission.
use std::{error::Error, path::Path, process::Command};

pub fn run(arguments: &[String], dry_run: bool) -> Result<(), Box<dyn Error>> {
    let (mode, options) = arguments
        .split_first()
        .ok_or("matrix replay mode required")?;
    let mut source = None;
    let mut profile = None;
    let mut worker = None;
    let mut output = None;
    let mut remaining = options.iter();
    let capture = remaining.next().ok_or("capture path required")?;
    while let Some(key) = remaining.next() {
        let value = remaining.next().ok_or("replay option requires a value")?;
        let slot = match key.as_str() {
            "--source" => &mut source,
            "--profile" => &mut profile,
            "--worker" => &mut worker,
            "--output" => &mut output,
            _ => return Err(format!("unsupported replay option {key}").into()),
        };
        if slot.replace(value).is_some() {
            return Err(format!("duplicate replay option {key}").into());
        }
    }
    let source = source.ok_or("explicit expected --source required")?;
    if source.len() != 40
        || !source
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("expected source must be a full lowercase Git commit SHA".into());
    }
    let worker = worker.ok_or("explicit retained --worker required")?;
    let factory = matches!(
        mode.as_str(),
        "--verify-compact-factory-controls" | "--check-compact-factory-verifier"
    );
    let projection = matches!(
        mode.as_str(),
        "--verify-matrix-projection" | "--check-matrix-projection-verifier"
    );
    if (projection || factory) && profile.is_some() {
        return Err("this replay does not accept a FARGAN profile".into());
    }
    let profile = profile.map(String::as_str).unwrap_or("matrix-local");
    if !matches!(profile, "matrix-local" | "compact-local" | "source-cache") {
        return Err("unsupported replay profile".into());
    }
    let check = match mode.as_str() {
        "--verify-fargan-capture"
        | "--verify-matrix-projection"
        | "--verify-compact-factory-controls" => false,
        "--check-fargan-verifier"
        | "--check-matrix-projection-verifier"
        | "--check-compact-factory-verifier" => true,
        _ => return Err("unsupported capture replay mode".into()),
    };
    let script = if factory {
        if check {
            "check-compact-factory-negatives.py".to_owned()
        } else {
            "verify-compact-linear-factory-controls.py".to_owned()
        }
    } else if projection {
        if check {
            "check-matrix-projection-negatives.py".to_owned()
        } else {
            "verify-matrix-projection-capture.py".to_owned()
        }
    } else if check {
        "check-fargan-capture-negatives.py".to_owned()
    } else {
        format!("verify-{profile}-public.py")
    };
    if dry_run {
        println!(
            "Numerical diagnostic replay: {script}; source {source}; no timing or release acceptance"
        );
        return Ok(());
    }
    let root = crate::workspace::workspace_root()?;
    execute(
        &root,
        &script,
        capture,
        source,
        worker,
        output,
        (check && !projection && !factory).then_some(profile),
    )
}

fn execute(
    root: &Path,
    script: &str,
    capture: &str,
    source: &str,
    worker: &str,
    output: Option<&String>,
    profile: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("python3");
    command
        .current_dir(root)
        .arg(root.join("proof/performance").join(script))
        .arg(capture)
        .args(["--source", source, "--worker", worker]);
    if let Some(profile) = profile {
        command.args(["--profile", profile]);
    }
    if let Some(output) = output {
        command.args(["--output", output]);
    }
    if !command.status()?.success() {
        return Err("capture replay refused; inspect verifier output".into());
    }
    Ok(())
}

/// Compare two independently verified captures with explicit expected identities.
pub fn compare(arguments: &[String], dry_run: bool) -> Result<(), Box<dyn Error>> {
    let before = arguments.get(1).ok_or("before capture required")?;
    let mut against = None;
    let mut source = None;
    let mut worker = None;
    let mut against_source = None;
    let mut against_worker = None;
    let mut output = None;
    for pair in arguments[2..].chunks(2) {
        let [key, value] = pair else {
            return Err("comparison option requires a value".into());
        };
        let slot = match key.as_str() {
            "--against" => &mut against,
            "--source" => &mut source,
            "--worker" => &mut worker,
            "--against-source" => &mut against_source,
            "--against-worker" => &mut against_worker,
            "--output" => &mut output,
            _ => return Err(format!("unknown comparison option {key}").into()),
        };
        if slot.replace(value).is_some() {
            return Err(format!("duplicate comparison option {key}").into());
        }
    }
    let against = against.ok_or("explicit --against capture required")?;
    let source = source.ok_or("explicit --source required")?;
    let worker = worker.ok_or("explicit --worker required")?;
    let against_source = against_source.ok_or("explicit --against-source required")?;
    let against_worker = against_worker.ok_or("explicit --against-worker required")?;
    for identity in [source, against_source] {
        if identity.len() != 40
            || !identity
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("comparison requires full lowercase commit SHAs".into());
        }
    }
    if dry_run {
        println!("source-cache diagnostic comparison; no timing acceptance");
        return Ok(());
    }
    let root = crate::workspace::workspace_root()?;
    let mut command = Command::new("python3");
    command
        .current_dir(&root)
        .arg(
            root.join("proof/performance").join(
                if arguments
                    .first()
                    .is_some_and(|a| a == "--check-source-cache-comparison")
                {
                    "check-source-cache-equivalence.py"
                } else {
                    "verify-source-cache-equivalence.py"
                },
            ),
        )
        .args([before, against])
        .args([
            "--before-source",
            source,
            "--after-source",
            against_source,
            "--before-worker",
            worker,
            "--after-worker",
            against_worker,
        ]);
    if let Some(output) = output {
        command.args(["--output", output]);
    }
    if !command.status()?.success() {
        return Err("source-cache comparison refused".into());
    }
    Ok(())
}
