use crate::cli::GlobalOpts;
use std::process::Command;

pub fn run(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!("would run pinned Tongues text-to-speech starter as degraded WAV artifact");
        }
        return Ok(());
    }
    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--package",
        "conduit-tongues",
        "--features",
        "speech",
        "--bin",
        "conduit-tongues-demo",
    ]);
    if opts.locked {
        command.arg("--locked");
    }
    command.arg("--");
    if opts.json {
        command.arg("--json");
    }
    if opts.quiet {
        command.arg("--quiet");
    }
    let status = command.status()?;
    if !status.success() {
        return Err(format!("conduit-tongues demo exited with {status}").into());
    }
    Ok(())
}
