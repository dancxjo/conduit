use std::{path::Path, process::Command};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ExecutedCommand {
    pub purpose: String,
    pub cwd: String,
    pub program: String,
    pub args: Vec<String>,
}

impl ExecutedCommand {
    pub fn new(
        purpose: impl Into<String>,
        cwd: &Path,
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            purpose: purpose.into(),
            cwd: cwd.display().to_string(),
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

pub fn run_capture(
    specification: ExecutedCommand,
    commands: &mut Vec<ExecutedCommand>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    commands.push(specification.clone());
    let output = Command::new(&specification.program)
        .args(&specification.args)
        .current_dir(&specification.cwd)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "{} failed with {}: {}",
            display_argv(&specification),
            output.status,
            bounded_tail(&output.stderr, 8192)
        )
        .into());
    }
    Ok(output.stdout)
}

pub fn cargo_tree(package_root: &Path, features: &[String], purpose: &str) -> ExecutedCommand {
    ExecutedCommand::new(
        purpose,
        package_root,
        "cargo",
        [
            "tree".to_owned(),
            "--locked".to_owned(),
            "--edges".to_owned(),
            "normal".to_owned(),
            "--prefix".to_owned(),
            "none".to_owned(),
            "--no-default-features".to_owned(),
            "--features".to_owned(),
            features.join(","),
        ],
    )
}

pub fn cargo_fmt(repo_root: &Path, manifest: &Path, purpose: &str) -> ExecutedCommand {
    ExecutedCommand::new(
        purpose,
        repo_root,
        "cargo",
        [
            "fmt".to_owned(),
            "--manifest-path".to_owned(),
            manifest.display().to_string(),
            "--".to_owned(),
            "--check".to_owned(),
        ],
    )
}

pub fn cargo_check(package_root: &Path, features: &[String]) -> ExecutedCommand {
    ExecutedCommand::new(
        "compile-minimal-feature-closure",
        package_root,
        "cargo",
        [
            "+esp".to_owned(),
            "check".to_owned(),
            "--release".to_owned(),
            "--locked".to_owned(),
            "--no-default-features".to_owned(),
            "--features".to_owned(),
            features.join(","),
        ],
    )
}

pub fn cargo_build(package_root: &Path, features: &[String]) -> ExecutedCommand {
    ExecutedCommand::new(
        "build-full-feature-artifact",
        package_root,
        "cargo",
        [
            "+esp".to_owned(),
            "build".to_owned(),
            "--release".to_owned(),
            "--locked".to_owned(),
            "--no-default-features".to_owned(),
            "--features".to_owned(),
            features.join(","),
        ],
    )
}

fn display_argv(command: &ExecutedCommand) -> String {
    std::iter::once(command.program.as_str())
        .chain(command.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

fn bounded_tail(bytes: &[u8], maximum: usize) -> String {
    let start = bytes.len().saturating_sub(maximum);
    String::from_utf8_lossy(&bytes[start..]).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_commands_are_locked_explicit_and_feature_derived() {
        let root = Path::new("/repo/firmware/esp32");
        let features = vec!["bluetooth".into(), "kernel-signal".into()];
        for command in [cargo_check(root, &features), cargo_build(root, &features)] {
            assert_eq!(command.program, "cargo");
            assert_eq!(command.args[0], "+esp");
            assert!(command.args.contains(&"--locked".into()));
            assert!(command.args.contains(&"--no-default-features".into()));
            assert_eq!(command.args.last().unwrap(), "bluetooth,kernel-signal");
            assert!(!command
                .args
                .iter()
                .any(|arg| arg == "--jobs" || arg == "-j"));
        }
    }

    #[test]
    fn formatting_check_is_manifest_exact_and_does_not_set_jobs() {
        let command = cargo_fmt(
            Path::new("/repo"),
            Path::new("/repo/firmware/esp32/Cargo.toml"),
            "format-firmware",
        );
        assert_eq!(
            command.args,
            [
                "fmt",
                "--manifest-path",
                "/repo/firmware/esp32/Cargo.toml",
                "--",
                "--check",
            ]
        );
    }
}
