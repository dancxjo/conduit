//! Repeatable local compositor differential and optimized memory-scanout timings.
use super::ConduitosError;
use crate::cli::GlobalOpts;
use std::process::Command;

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let arguments = [
        "test",
        "--locked",
        "--config",
        "profile.dev.package.conduitos.opt-level=3",
        "--config",
        "profile.dev.debug=0",
        "--config",
        "profile.dev.incremental=false",
        "-p",
        "conduitos",
        "--features",
        "native-compositor",
        "--lib",
        "native_compositor::frame_composition",
        "--",
        "--include-ignored",
        "--test-threads=1",
        "--nocapture",
    ];
    if opts.dry_run {
        println!("cargo {}", arguments.join(" "));
        return Ok(());
    }
    let status = Command::new("cargo")
        .args(arguments)
        .status()
        .map_err(|error| {
            ConduitosError::refusal("compositor-performance-start", error.to_string())
        })?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "compositor-performance-failed",
            status.to_string(),
        ));
    }
    println!("Compositor and reference use opt-level 3 with debug assertions. Raw wall batches include memory target checks; no QEMU, physical display, input-latency or universal speedup claim.");
    Ok(())
}
