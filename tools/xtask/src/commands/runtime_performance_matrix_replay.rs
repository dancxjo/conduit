//! Independent retained capture replay; never a timing or release admission.
use std::{error::Error, path::Path, process::Command};

pub fn run(arguments: &[String], dry_run: bool) -> Result<(), Box<dyn Error>> {
    let (mode, options) = arguments
        .split_first()
        .ok_or("matrix replay mode required")?;
    let mut source = None;
    let mut worker = None;
    let mut output = None;
    let mut remaining = options.iter();
    let capture = remaining.next().ok_or("capture path required")?;
    while let Some(key) = remaining.next() {
        let value = remaining.next().ok_or("replay option requires a value")?;
        let slot = match key.as_str() {
            "--source" => &mut source,
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
    let script = match mode.as_str() {
        "--verify-matrix-local" => "verify-matrix-local-public.py",
        "--check-matrix-local-verifier" => "check-matrix-local-negatives.py",
        _ => return Err("unsupported matrix replay mode".into()),
    };
    if dry_run {
        println!("matrix-local diagnostic replay: {script}; source {source}; no timing or release acceptance");
        return Ok(());
    }
    let root = crate::workspace::workspace_root()?;
    execute(&root, script, capture, source, worker, output)
}

fn execute(
    root: &Path,
    script: &str,
    capture: &str,
    source: &str,
    worker: &str,
    output: Option<&String>,
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("python3");
    command
        .current_dir(root)
        .arg(root.join("proof/performance").join(script))
        .arg(capture)
        .args(["--source", source, "--worker", worker]);
    if let Some(output) = output {
        command.args(["--output", output]);
    }
    if !command.status()?.success() {
        return Err("matrix-local replay refused; inspect verifier output".into());
    }
    Ok(())
}
