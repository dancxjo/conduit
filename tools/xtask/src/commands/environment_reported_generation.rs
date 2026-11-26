//! Complete original-generation codec and typed Face preparation; no live proof.
use std::{error::Error, io::Write, path::Path, process::Command};

pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    tests(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "conduit-dht11-observation",
            "--features",
            "reported-generation",
            "--test",
            "reported_generation",
        ],
        6,
    )?;
    tests(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "conduit-face",
            "--lib",
            "quantity_observation_face::tests",
        ],
        3,
    )?;
    tests(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "conduit-face",
            "--test",
            "quantity_observation_contribution",
        ],
        6,
    )?;
    tests(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--lib",
            "installed_std::reported_generation_route::tests",
            "--",
            "--test-threads=1",
        ],
        2,
    )?;
    tests(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--lib",
            "installed_std::reported_generation_factory::tests",
            "--",
            "--test-threads=1",
        ],
        1,
    )?;
    for package in ["conduit-dht11-observation", "conduit-face"] {
        let mut args = vec!["clippy", "--locked", "-p", package, "--lib", "--tests"];
        if package == "conduit-dht11-observation" {
            args.extend(["--features", "reported-generation"]);
        }
        args.extend(["--", "-D", "warnings"]);
        super::execute(Command::new("cargo").current_dir(root).args(args))?;
    }
    for target in ["thumbv6m-none-eabi", "wasm32-unknown-unknown"] {
        super::execute(Command::new("cargo").current_dir(root).args([
            "check",
            "--locked",
            "-p",
            "conduit-dht11-observation",
            "--no-default-features",
            "--features",
            "reported-generation",
            "--target",
            target,
        ]))?;
    }
    super::execute(Command::new("cargo").current_dir(root).args([
        "check",
        "--locked",
        "-p",
        "conduit-face",
        "--no-default-features",
        "--target",
        "wasm32-unknown-unknown",
    ]))
}
fn tests(root: &Path, args: &[&str], minimum: usize) -> Result<(), Box<dyn Error>> {
    let result = Command::new("cargo")
        .current_dir(root)
        .args(args)
        .output()?;
    std::io::stdout().write_all(&result.stdout)?;
    std::io::stderr().write_all(&result.stderr)?;
    if !result.status.success() {
        return Err(format!("reported generation proof failed: {}", result.status).into());
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    let ran = stdout.lines().any(|line| {
        line.strip_prefix("test result: ok. ")
            .and_then(|line| line.split_once(" passed;"))
            .is_some_and(|(count, rest)| {
                count.parse::<usize>().is_ok_and(|count| count >= minimum)
                    && rest.starts_with(" 0 failed; 0 ignored;")
            })
    });
    if !ran {
        return Err("reported generation proof skipped required unignored tests".into());
    }
    Ok(())
}
