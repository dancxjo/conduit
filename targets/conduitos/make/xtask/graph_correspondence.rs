//! Opt-in CPU conformance; no images, browser process, device or network.
use super::ConduitosError;
use crate::cli::GlobalOpts;
use std::process::Command;

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let arguments = [
        "test",
        "--locked",
        "-p",
        "conduitos",
        "--lib",
        "--features",
        "native-compositor",
        "native_browser_and_linear_reconstruct_one_exact_graph_with_explicit_gaps",
        "--",
        "--ignored",
        "--test-threads=1",
    ];
    println!("cargo {}", arguments.join(" "));
    if opts.dry_run {
        return Ok(());
    }
    let output = Command::new("cargo")
        .args(arguments)
        .output()
        .map_err(|error| {
            ConduitosError::refusal("graph-correspondence-start", error.to_string())
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("{stdout}");
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    if !output.status.success() || !stdout.contains("1 passed; 0 failed;") {
        return Err(ConduitosError::refusal(
            "graph-correspondence-failed",
            "expected one successful opt-in conformance case",
        ));
    }
    Ok(())
}
