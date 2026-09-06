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

pub fn run_research(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!("would run bounded Tongues paired-latent research capstone");
        }
        return Ok(());
    }
    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--package",
        "conduit-tongues",
        "--bin",
        "conduit-tongues-research",
    ]);
    if opts.locked {
        command.arg("--locked");
    }
    let status = command.status()?;
    if !status.success() {
        return Err(format!("Tongues research capstone exited with {status}").into());
    }
    Ok(())
}

pub fn run_analysis(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!("would analyze the frozen Tongues latent dynamics with bounded controls");
        }
        return Ok(());
    }
    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--package",
        "conduit-tongues",
        "--bin",
        "conduit-tongues-analysis",
    ]);
    if opts.locked {
        command.arg("--locked");
    }
    let status = command.status()?;
    if !status.success() {
        return Err(format!("Tongues dynamics analysis exited with {status}").into());
    }
    Ok(())
}
