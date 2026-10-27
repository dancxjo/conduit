//! Attended real first-host/MCU acceptance and comparative Pico memory gate.
use std::{error::Error, path::Path, process::Command};
pub(super) fn dispatch(root: &Path, args: &[String]) -> Option<Result<(), Box<dyn Error>>> {
    let words = args.iter().map(String::as_str).collect::<Vec<_>>();
    match words.as_slice() {
        ["network-workspace-check-image", key] => Some(
            super::environment_esp32::build_network_workspace_check(root, Path::new(key)),
        ),
        ["flat-network-appliance-image", key] => Some(
            super::environment_esp32::build_flat_network_appliance(root, Path::new(key)),
        ),
        ["biography-maintenance-image", key] => Some(
            super::environment_esp32::build_biography_maintenance(root, Path::new(key)),
        ),
        ["local-appliance-image", key] => Some(super::environment_esp32::build_local_appliance(
            root,
            Path::new(key),
        )),
        ["verify-carrier-recovery", pid, run, log, dropped] => Some(super::execute(
            Command::new("python3")
                .current_dir(root)
                .arg(root.join("bodies/environment/host/proof/carrier_recovery_verify.py"))
                .args([pid, run, log, dropped])
                .env("PYTHONDONTWRITEBYTECODE", "1"),
        )),
        ["owner-carrier-outage", upstream, state, resume, output] => {
            Some(carrier_outage(root, upstream, state, resume, output))
        }
        ["observation-history-outage", pid, run, url] => Some(super::execute(
            Command::new("python3")
                .current_dir(root)
                .arg(root.join("bodies/environment/host/proof/local_evidence_outage.py"))
                .args(["--history", pid, run, url]),
        )),
        ["local-evidence-outage", pid, run, url] => Some(super::execute(
            Command::new("python3")
                .current_dir(root)
                .arg(root.join("bodies/environment/host/proof/local_evidence_outage.py"))
                .args([pid, run, url]),
        )),
        ["member-image", key, "--local-evidence-check"] => Some(
            super::environment_esp32::build_local_evidence(root, Path::new(key)),
        ),
        ["host-owned", port, state, limit, "--display-check", resume, history]
            if resume.starts_with("--resume=") && history.starts_with("--history-peer=") =>
        {
            Some(host(
                root,
                &[port, state, limit, resume, history, "--display-check"],
            ))
        }
        ["host-owned", port, state, limit, "--display-check", history]
            if history.starts_with("--history-peer=") =>
        {
            Some(host(
                root,
                &[port, state, limit, history, "--display-check"],
            ))
        }
        ["host-owned", port, state, limit, "--display-check", resume]
            if resume.starts_with("--resume=") =>
        {
            Some(host(root, &[port, state, limit, resume, "--display-check"]))
        }
        ["member-image", key, "--wireless-display-check"] => Some(
            super::environment_esp32::build_wireless_display(root, Path::new(key)),
        ),
        ["owner-key", state] => Some(host(root, &["--provision", state])),
        ["host-owned", port, state, "--mask-prepare"] => {
            Some(host(root, &[port, state, "--mask-prepare"]))
        }
        ["host-owned", port, state, limit, "--mask-prepare"] => {
            Some(host(root, &[port, state, limit, "--mask-prepare"]))
        }
        ["member-image", key, "--mask-prepare"] => Some(
            super::environment_esp32::build_mask_preparation(root, Path::new(key)),
        ),
        ["host-owned", port, state, "--display-check"] => {
            Some(host(root, &[port, state, "--display-check"]))
        }
        ["host-owned", port, state, limit, "--display-check"] => {
            Some(host(root, &[port, state, limit, "--display-check"]))
        }
        ["host-owned", port, state] => Some(host(root, &[port, state])),
        ["host-owned", port, state, limit] => Some(host(root, &[port, state, limit])),
        ["member-image", key] => Some(super::environment_esp32::build_member(root, Path::new(key))),
        ["member-image", key, "--display-fit"] => Some(
            super::environment_esp32::build_display_fit(root, Path::new(key)),
        ),
        ["member-image", key, "--display-check"] => Some(
            super::environment_esp32::build_display_check(root, Path::new(key)),
        ),
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

fn carrier_outage(
    root: &Path,
    upstream: &str,
    state: &str,
    resume: &str,
    output: &str,
) -> Result<(), Box<dyn Error>> {
    super::execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "build",
                "--locked",
                "--manifest-path",
                "bodies/environment/host/Cargo.toml",
            ])
            .env("CARGO_TARGET_DIR", root.join("target")),
    )?;
    super::execute(
        Command::new("python3")
            .current_dir(root)
            .arg(root.join("bodies/environment/host/proof/acceptance_loss_test.py"))
            .env("PYTHONDONTWRITEBYTECODE", "1"),
    )?;
    super::execute(
        Command::new("python3")
            .current_dir(root)
            .arg(root.join("bodies/environment/host/proof/carrier_outage.py"))
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .args([upstream, state, resume])
            .arg(root.join("target/debug/conduit-environment-host"))
            .arg(output),
    )
}
