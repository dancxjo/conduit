//! Release construction of the selected native proof profile; no flashing.
use super::execute;
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path, arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let pulse = arguments == ["pulse-esp32"];
    let network = arguments == ["network-esp32"];
    let package = root.join(if network {
        "targets/esp32/network-switch"
    } else if pulse {
        "targets/esp32/pulse-capture"
    } else {
        "targets/esp32/proof/tm1637"
    });
    let toolchain: toml::Value = toml::from_str(&std::fs::read_to_string(
        package.join("rust-toolchain.toml"),
    )?)?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|v| v.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or("display diagnostic has no pinned toolchain")?;
    let mut command = Command::new("cargo");
    command
        .current_dir(&package)
        .arg(format!("+{channel}"))
        .args([
            if pulse || network { "check" } else { "build" },
            "--locked",
            "--release",
        ])
        .env("CARGO_TARGET_DIR", root.join("target/environment-esp32"));
    if pulse {
        command.args(["--features", "host-call"]);
    } else if arguments == ["capture-esp32"] {
        command.args(["--features", "capture"]);
    } else if arguments == ["root-face-esp32"] {
        command.args(["--features", "native-face-root"]);
    } else if arguments == ["root-esp32"] {
        command.args(["--features", "native-root"]);
    } else if arguments == ["ap-esp32"] {
        command.args(["--features", "ap"]);
    } else if arguments == ["live-esp32"] {
        command.args(["--features", "live"]);
    }
    execute(&mut command)
}
