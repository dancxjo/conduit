//! Repeatable local compositor differential and optimized memory-scanout timings.
use super::ConduitosError;
use crate::cli::GlobalOpts;
use std::process::Command;

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    // One Cargo invocation keeps both proof scopes on the same built test image.
    // Libtest accepts multiple filters as a union after Cargo's argument boundary.
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
        "--",
        "--include-ignored",
        "--test-threads=1",
        "--nocapture",
        "native_compositor::frame_composition",
        "display::raw",
    ];
    println!("cargo {}", arguments.join(" "));
    if !opts.dry_run {
        let output = Command::new("cargo")
            .args(arguments)
            .output()
            .map_err(|error| {
                ConduitosError::refusal("compositor-performance-start", error.to_string())
            })?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        if !output.status.success() {
            return Err(ConduitosError::refusal(
                "compositor-performance-failed",
                output.status.to_string(),
            ));
        }
        for marker in ["span-compositor case=", "volatile-scanout mode="] {
            if !String::from_utf8_lossy(&output.stdout).contains(marker) {
                return Err(ConduitosError::refusal(
                    "compositor-performance-missing",
                    "successful test process did not emit the required measurement rows",
                ));
            }
        }
    }
    println!("Compositor and reference use opt-level 3 with debug assertions. Raw wall batches include memory target checks; no QEMU, physical display, input-latency or universal speedup claim.");
    Ok(())
}
