//! Actual finite local GPU experiment, never a capability advertisement.
pub(super) fn run(arguments: &[String], dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::workspace::workspace_root()?;
    let args = arguments
        .iter()
        .filter(|arg| arg.as_str() != "--gpu-feasibility")
        .collect::<Vec<_>>();
    if dry_run {
        println!("finite local CUDA projection feasibility: {args:?}; no GPU offer admitted");
        return Ok(());
    }
    let status = std::process::Command::new("python3")
        .arg(root.join("proof/performance/gpu/run.py"))
        .args(args)
        .current_dir(root)
        .status()?;
    if !status.success() {
        return Err(
            "GPU feasibility refused; retain the output's compiler/raw/refusal records".into(),
        );
    }
    Ok(())
}
