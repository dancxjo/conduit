use std::process::Command;

use crate::cli::{GlobalOpts, SetupArgs, SetupTarget};

pub fn run(args: SetupArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    match args.target {
        SetupTarget::LinuxRelease => setup_linux_release(opts),
    }
}

fn setup_linux_release(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(target_os = "linux") {
        return Err("linux-release setup must run on a Linux development host".into());
    }

    run(
        "install the pinned Rust aarch64 target",
        "rustup",
        &["target", "add", "aarch64-unknown-linux-gnu"],
        opts,
    )?;
    run_privileged(
        "refresh Debian/Ubuntu package metadata",
        "apt-get",
        &["update"],
        opts,
    )?;
    run_privileged(
        "install Linux release cross-build prerequisites",
        "apt-get",
        &[
            "install",
            "-y",
            "--no-install-recommends",
            "gcc-aarch64-linux-gnu",
            "libc6-dev-arm64-cross",
            "xvfb",
        ],
        opts,
    )?;

    if !opts.quiet {
        println!("Linux release prerequisites are installed.");
        println!("Verify them with: cargo xtask doctor linux-release");
    }
    Ok(())
}

fn run_privileged(
    description: &str,
    program: &str,
    args: &[&str],
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut sudo_args = Vec::with_capacity(args.len() + 1);
    sudo_args.push(program);
    sudo_args.extend_from_slice(args);
    run(description, "sudo", &sudo_args, opts)
}

fn run(
    description: &str,
    program: &str,
    args: &[&str],
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!("setup: {description}: {program} {}", args.join(" "));
        }
        return Ok(());
    }
    if !opts.quiet {
        println!("setup: {description}");
    }
    let status = Command::new(program).args(args).status().map_err(|error| {
        format!("launch setup command {program} for {description}: {error}")
    })?;
    if !status.success() {
        return Err(format!("{description} failed with {status}").into());
    }
    Ok(())
}
