//! Repeatable local compositor differential and optimized memory-scanout timings.
use super::ConduitosError;
use crate::cli::GlobalOpts;
use std::process::Command;

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    for (filter, marker) in [
        (
            "native_compositor::frame_composition",
            "span-compositor case=",
        ),
        ("display::raw", "volatile-scanout mode="),
    ] {
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
            filter,
            "--",
            "--include-ignored",
            "--test-threads=1",
            "--nocapture",
        ];
        println!("cargo {}", arguments.join(" "));
        if opts.dry_run {
            continue;
        }
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
        if !String::from_utf8_lossy(&output.stdout).contains(marker) {
            return Err(ConduitosError::refusal(
                "compositor-performance-missing",
                "successful test process did not emit the required measurement rows",
            ));
        }
    }
    println!("Compositor and reference use opt-level 3 with debug assertions. Raw wall batches include memory target checks; no QEMU, physical display, input-latency or universal speedup claim.");
    Ok(())
}
