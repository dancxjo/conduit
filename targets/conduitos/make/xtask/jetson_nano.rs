//! Native Nano image build with retained NVIDIA firmware partitions.
use super::{ConduitosArch, ConduitosError, profile::Paths, report::git_head};
use crate::cli::GlobalOpts;
use clap::Args;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Args, Debug)]
pub struct ImageArgs {
    /// Previously acquired scrubbed NVIDIA donor; otherwise acquire the official base.
    #[arg(long)]
    donor: Option<PathBuf>,
    /// Compressed image output. Existing files are refused.
    #[arg(long)]
    output: PathBuf,
}

fn run(command: &mut Command) -> Result<(), ConduitosError> {
    let status = command.status().map_err(|error| {
        ConduitosError::refusal("jetson-command-unavailable", error.to_string())
    })?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "jetson-command-failed",
            format!("{command:?}: {status}"),
        ));
    }
    Ok(())
}

pub fn execute(args: ImageArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let stage = paths.cargo_target.join("jetson-nano");
    let donor = args
        .donor
        .unwrap_or_else(|| stage.join("nvidia-jp461-firmware-donor.img"));
    let image = stage.join("Image");
    if opts.dry_run {
        println!(
            "build native Nano kernel, preserve NVIDIA firmware from {}, assemble {}",
            donor.display(),
            args.output.display()
        );
        return Ok(());
    }
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(&paths.root)
        .output()
        .map_err(|error| ConduitosError::refusal("jetson-source-status", error.to_string()))?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err(ConduitosError::refusal(
            "jetson-source-dirty",
            "commit the exact image source before building a downloadable artifact",
        ));
    }
    let source = git_head(&paths.root)?;
    std::fs::create_dir_all(&stage)
        .map_err(|error| ConduitosError::refusal("jetson-stage", error.to_string()))?;
    let tooling = paths.root.join("targets/jetson-nano/make");
    if !donor.exists() {
        run(Command::new("python3")
            .arg(tooling.join("donor.py"))
            .arg("--output")
            .arg(&donor)
            .current_dir(&paths.root))?;
    }
    let mut receipt_name = donor.as_os_str().to_owned();
    receipt_name.push(".json");
    if !Path::new(&receipt_name).is_file() {
        return Err(ConduitosError::refusal(
            "jetson-donor-receipt-missing",
            "use the verified official donor acquisition receipt",
        ));
    }
    run(Command::new("cargo")
        .args([
            "build",
            "--locked",
            "-p",
            "conduitos",
            "--bin",
            "conduitos-aarch64-jetson-nano",
            "--features",
            "aarch64-jetson-nano",
            "--target",
            "aarch64-unknown-none",
            "--release",
        ])
        .env("CARGO_TARGET_DIR", &paths.cargo_target)
        .env("CONDUITOS_BUILD_ID", format!("build:jetson-nano:{source}"))
        .env("CONDUITOS_IMAGE_ID", format!("image:jetson-nano:{source}"))
        .current_dir(&paths.root))?;
    let license = stage.join("NVIDIA-LICENSE.txt");
    if !license.is_file() {
        run(Command::new("curl").args(["--fail", "--location", "--max-filesize", "2097152", "--output"])
            .arg(&license).arg("https://developer.download.nvidia.com/embedded/L4T/r32_Release_v7.1/T186/Tegra_Software_License_Agreement-Tegra-Linux.txt"))?;
    }
    let elf = paths
        .cargo_target
        .join("aarch64-unknown-none/release/conduitos-aarch64-jetson-nano");
    run(Command::new("aarch64-linux-gnu-objcopy")
        .args(["-O", "binary"])
        .arg(&elf)
        .arg(&image))?;
    package(&tooling, &donor, &image, &args.output, &source, &license)
}

fn package(
    tooling: &Path,
    donor: &Path,
    image: &Path,
    output: &Path,
    source: &str,
    license: &Path,
) -> Result<(), ConduitosError> {
    run(Command::new("python3")
        .arg(tooling.join("media.py"))
        .arg("--donor")
        .arg(donor)
        .arg("--image")
        .arg(image)
        .arg("--output")
        .arg(output)
        .arg("--firmware-license")
        .arg(license)
        .arg("--source-sha")
        .arg(source)
        .args(["--firmware-release", "JetPack 4.6.1 / L4T R32.7.1"]))
}
