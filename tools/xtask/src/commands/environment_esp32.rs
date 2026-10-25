//! Pinned ESP32 firmware construction; no flash or installed Body claim.
use std::{
    error::Error,
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn build(root: &Path, native: bool, body: bool) -> Result<(), Box<dyn Error>> {
    let package = root.join("targets/esp32/proof/tm1637");
    let toolchain: toml::Value = toml::from_str(&std::fs::read_to_string(
        package.join("rust-toolchain.toml"),
    )?)?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|value| value.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or("ESP32 firmware has no pinned toolchain")?;
    let linker = linker(channel)?;
    let mut paths = vec![linker.parent().ok_or("linker directory")?.to_path_buf()];
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    let mut command = Command::new("cargo");
    command
        .current_dir(&package)
        .arg(format!("+{channel}"))
        .args(["build", "--locked", "--release"])
        .env("PATH", std::env::join_paths(paths)?)
        .env("CARGO_TARGET_XTENSA_ESP32_NONE_ELF_LINKER", &linker)
        .env("CARGO_TARGET_DIR", root.join("target/environment-esp32"));
    if native {
        command.args([
            "--features",
            if body {
                "body-image-root"
            } else {
                "native-face-root"
            },
        ]);
    }
    super::execute(&mut command)
}

fn linker(channel: &str) -> Result<PathBuf, Box<dyn Error>> {
    let output = Command::new("rustup")
        .args(["run", channel, "rustc", "--print", "sysroot"])
        .output()?;
    if !output.status.success() {
        return Err("cannot inspect pinned ESP32 toolchain; run its setup first".into());
    }
    let sysroot = PathBuf::from(String::from_utf8(output.stdout)?.trim());
    let directory = sysroot.join("xtensa-esp-elf");
    let executable = format!("xtensa-esp32-elf-gcc{}", std::env::consts::EXE_SUFFIX);
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(&directory)? {
        let candidate = entry?.path().join("xtensa-esp-elf/bin").join(&executable);
        if candidate.is_file() {
            candidates.push(candidate);
        }
    }
    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err("pinned ESP32 toolchain has no installed Xtensa linker".into()),
        _ => Err("pinned ESP32 toolchain has ambiguous Xtensa linkers".into()),
    }
}
