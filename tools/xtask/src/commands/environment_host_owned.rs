//! Attended real first-host/MCU acceptance and comparative Pico memory gate.
use std::{error::Error, path::Path, process::Command};
pub(super) fn dispatch(root: &Path, args: &[String]) -> Option<Result<(), Box<dyn Error>>> {
    let words = args.iter().map(String::as_str).collect::<Vec<_>>();
    match words.as_slice() {
        ["owner-key", state] => Some(host(root, &["--provision", state])),
        ["host-owned", port, state] => Some(host(root, &[port, state])),
        ["host-owned", port, state, limit] => Some(host(root, &[port, state, limit])),
        ["member-image", key] => Some(super::environment_esp32::build_member(root, Path::new(key))),
        ["pico-maximal", birth] => Some(pico(root, birth)),
        _ => None,
    }
}
fn host(root: &Path, args: &[&str]) -> Result<(), Box<dyn Error>> {
    super::execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "run",
                "--locked",
                "--manifest-path",
                "bodies/environment/host/Cargo.toml",
                "--",
            ])
            .args(args)
            .env("CARGO_TARGET_DIR", root.join("target")),
    )
}
fn pico(root: &Path, birth: &str) -> Result<(), Box<dyn Error>> {
    let birth = std::fs::canonicalize(birth)?;
    let target = root.join("target/environment-rp2040");
    super::execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "build",
                "--locked",
                "--release",
                "--target",
                "thumbv6m-none-eabi",
                "--features",
                "maximal-target",
                "--manifest-path",
                "targets/rp2040/proof/dht11/Cargo.toml",
            ])
            .env("CARGO_TARGET_DIR", &target)
            .env("CONDUIT_ENVIRONMENT_BODY_BIRTH_SIGN", birth),
    )?;
    let elf = target.join("thumbv6m-none-eabi/release/conduit-pico-dht11-probe");
    let uf2 = elf.with_extension("uf2");
    super::execute(Command::new("elf2uf2-rs").arg(&elf).arg(&uf2))?;
    println!("Pico memory checkpoint: {}", uf2.display());
    Ok(())
}
