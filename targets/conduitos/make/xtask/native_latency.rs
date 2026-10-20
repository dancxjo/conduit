//! Source-bound process observation around the ordinary graphical journey.
use super::{image, profile::Paths, report::git_head, ConduitosArch, ConduitosError};
use crate::cli::GlobalOpts;
use serde_json::json;
use std::{fs, path::Path, process::Command};

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(refusal(
            "native latency observation needs a real image and guest",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    require_clean_source(&paths.root)?;
    let source = git_head(&paths.root)?;
    let rustc = tool_version("rustc", &["--version", "--verbose"])?;
    let qemu = tool_version("qemu-system-x86_64", &["--version"])?;
    let started = std::time::Instant::now();
    let built = image::execute_native_latency(opts)?;
    let preparation_ns = started.elapsed().as_nanos();
    let identity =
        super::journey_proof::execute_supplied(opts, &paths.iso, built.iso_sha256.clone())?;
    let total_ns = started.elapsed().as_nanos();
    if git_head(&paths.root)? != source {
        return Err(refusal("source changed during native observation"));
    }
    require_clean_source(&paths.root)?;
    let serial = fs::read_to_string(paths.target.join("journey-serial.log")).map_err(io_error)?;
    let guest = super::native_timing::parse(&serial)?;
    let records = guest["records"]
        .as_array()
        .ok_or_else(|| refusal("instrumented guest produced no timing records"))?;
    for required in [
        "boot-entry",
        "first-framebuffer-commit",
        "plot-build",
        "plan-seal",
        "workset-prepare",
        "kernel-prepare",
        "play-start",
        "play-input",
    ] {
        if !records.iter().any(|record| record["stage"] == required) {
            return Err(refusal(
                "instrumented journey omitted a required timing stage",
            ));
        }
    }
    fs::write(paths.target.join("journey-process-timing.json"), serde_json::to_vec_pretty(&json!({
        "schema":"conduit.conduitos/native-process-timing@1", "proof_class":"freestanding-emulator",
        "source_commit":source,"rustc":rustc,"qemu":qemu,
        "image_sha256":built.iso_sha256,"profile_id":identity.profile_id,
        "build_id":identity.build_id,"host_id":identity.host_id,"boot_id":identity.boot_id,
        "clock":"std::time::Instant","units":"nanoseconds","build_and_image_ns":preparation_ns,
        "journey_ns":total_ns - preparation_ns,"total_ns":total_ns,"guest":guest,
        "protocol":"One fresh QEMU process and boot; compiler and host OS caches are uncontrolled. Preparation includes supported image build. Journey includes emulator startup, inputs, receipt polling, frame capture and teardown. The process timer excludes xtask compilation and receipt serialization. Repeat records separately; this is not cold-machine, calibrated, or physical latency."
    })).expect("serializable timing report")).map_err(io_error)
}

fn refusal(detail: &str) -> ConduitosError {
    ConduitosError::refusal("native-latency-observation", detail)
}
fn io_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("native-latency-observation-io", error.to_string())
}

fn require_clean_source(root: &Path) -> Result<(), ConduitosError> {
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(root)
        .output()
        .map_err(io_error)?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err(refusal(
            "source-bound observation requires a clean checkout",
        ));
    }
    Ok(())
}
fn tool_version(tool: &str, arguments: &[&str]) -> Result<String, ConduitosError> {
    let output = Command::new(tool)
        .args(arguments)
        .output()
        .map_err(io_error)?;
    if !output.status.success() || output.stdout.is_empty() || output.stdout.len() > 4096 {
        return Err(refusal("tool identity unavailable"));
    }
    String::from_utf8(output.stdout).map_err(|_| refusal("tool identity is not UTF-8"))
}
