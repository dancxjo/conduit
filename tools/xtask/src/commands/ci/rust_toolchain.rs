//! Cheap fail-closed verification of the repository's exact generic Rust tools.

use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run(root: &Path) -> Result<(), String> {
    let contract = fs::read_to_string(root.join("rust-toolchain.toml"))
        .map_err(|error| format!("read rust-toolchain.toml: {error}"))?;
    let version = exact_channel(&contract)?;
    let rustc = output("rustc", &["--version", "--verbose"])?;
    let clippy = output("cargo", &["clippy", "--version"])?;
    let rustfmt = output("rustfmt", &["--version"])?;
    let active = output("rustup", &["show", "active-toolchain"])?;
    validate(&version, &rustc, &clippy, &rustfmt, &active)?;
    println!("CONDUIT_RUST_TOOLCHAIN={version}");
    println!("CONDUIT_RUSTC={}", rustc.lines().next().unwrap_or_default());
    println!("CONDUIT_CLIPPY={}", clippy.trim());
    println!("CONDUIT_RUSTFMT={}", rustfmt.trim());
    println!("CONDUIT_ACTIVE_TOOLCHAIN={}", active.trim());
    Ok(())
}

fn exact_channel(contract: &str) -> Result<String, String> {
    let values: Vec<_> = contract
        .lines()
        .filter_map(|line| line.trim().strip_prefix("channel = \"")?.strip_suffix('"'))
        .collect();
    if values.len() != 1
        || values[0].split('.').count() != 3
        || values[0]
            .split('.')
            .any(|part| part.parse::<u32>().is_err())
    {
        return Err("rust-toolchain.toml must declare one exact numeric channel".into());
    }
    Ok(values[0].to_owned())
}

fn output(program: &str, arguments: &[&str]) -> Result<String, String> {
    let result = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("run {program}: {error}"))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!(
            "{program} refused with {}: {}",
            result.status,
            stderr.trim()
        ));
    }
    String::from_utf8(result.stdout).map_err(|_| format!("{program} emitted non-UTF-8 identity"))
}

fn validate(
    version: &str,
    rustc: &str,
    clippy: &str,
    rustfmt: &str,
    active: &str,
) -> Result<(), String> {
    if !rustc.starts_with(&format!("rustc {version} ")) {
        return Err(format!(
            "Rust mismatch: expected {version}, observed {}",
            rustc.lines().next().unwrap_or("missing")
        ));
    }
    let minor = version
        .split('.')
        .nth(1)
        .ok_or("exact Rust minor is absent")?;
    if !clippy.starts_with(&format!("clippy 0.1.{minor} ")) {
        return Err(format!(
            "Clippy mismatch for Rust {version}: observed {}",
            clippy.trim()
        ));
    }
    if !rustfmt.starts_with("rustfmt ") {
        return Err(format!("rustfmt identity is malformed: {}", rustfmt.trim()));
    }
    if !active.starts_with(&format!("{version}-")) {
        return Err(format!(
            "active toolchain mismatch: expected {version}, observed {}",
            active.trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_contract_and_tools_match() {
        assert_eq!(
            exact_channel("[toolchain]\nchannel = \"1.98.1\"\n").unwrap(),
            "1.98.1"
        );
        validate(
            "1.98.1",
            "rustc 1.98.1 (abc 2026-01-01)\nrelease: 1.98.1\n",
            "clippy 0.1.98 (abc 2026-01-01)\n",
            "rustfmt 1.8.0-stable (abc 2026-01-01)\n",
            "1.98.1-x86_64-unknown-linux-gnu (overridden)\n",
        )
        .unwrap();
    }

    #[test]
    fn moving_channel_and_mismatched_components_refuse() {
        assert!(exact_channel("channel = \"stable\"").is_err());
        assert!(validate(
            "1.98.1",
            "rustc 1.99.0 (later 2026-02-01)\n",
            "clippy 0.1.99 (later 2026-02-01)\n",
            "rustfmt 1.9.0-stable (later 2026-02-01)\n",
            "stable-x86_64-unknown-linux-gnu\n",
        )
        .is_err());
    }
}
