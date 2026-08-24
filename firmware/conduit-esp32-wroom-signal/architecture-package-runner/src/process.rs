use std::{env, path::Path, process::Command};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ExecutedCommand {
    pub purpose: String,
    pub cwd: String,
    pub program: String,
    pub args: Vec<String>,
    pub environment: Vec<EnvironmentOverlay>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvironmentOverlay {
    pub variable: String,
    pub operation: String,
    pub value: String,
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
            environment: Vec::new(),
        }
    }

    pub fn with_path_prefix(mut self, prefix: &Path) -> Self {
        self.environment.push(EnvironmentOverlay {
            variable: "PATH".into(),
            operation: "prepend".into(),
            value: prefix.display().to_string(),
        });
        self
    }
}

pub fn run_capture(
    specification: ExecutedCommand,
    commands: &mut Vec<ExecutedCommand>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    commands.push(specification.clone());
    let mut command = Command::new(&specification.program);
    command
        .args(&specification.args)
        .current_dir(&specification.cwd);
    for overlay in &specification.environment {
        if overlay.variable != "PATH" || overlay.operation != "prepend" {
            return Err("only a recorded PATH-prefix environment overlay is supported".into());
        }
        let prefix = Path::new(&overlay.value);
        if !prefix.is_absolute() {
            return Err("recorded PATH prefix must be absolute".into());
        }
        let ambient = env::var_os("PATH").unwrap_or_default();
        let paths = std::iter::once(prefix.to_path_buf()).chain(env::split_paths(&ambient));
        command.env("PATH", env::join_paths(paths)?);
    }
    let output = command.output()?;
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

pub fn rustc_version(repo_root: &Path, toolchain_name: &str) -> ExecutedCommand {
    ExecutedCommand::new(
        "observe-esp-rust-toolchain",
        repo_root,
        "rustc",
        [format!("+{toolchain_name}"), "-Vv".to_owned()],
    )
}

pub fn rustc_sysroot(repo_root: &Path, toolchain_name: &str) -> ExecutedCommand {
    ExecutedCommand::new(
        "resolve-esp-rust-sysroot",
        repo_root,
        "rustc",
        [
            format!("+{toolchain_name}"),
            "--print".to_owned(),
            "sysroot".to_owned(),
        ],
    )
}

pub fn linker_version(repo_root: &Path, linker: &Path) -> ExecutedCommand {
    ExecutedCommand::new(
        "observe-esp-linker",
        repo_root,
        linker.display().to_string(),
        ["--version"],
    )
}

pub fn cargo_check(
    package_root: &Path,
    toolchain_name: &str,
    linker_bin: &Path,
    features: &[String],
) -> ExecutedCommand {
    ExecutedCommand::new(
        "compile-minimal-feature-closure",
        package_root,
        "cargo",
        [
            format!("+{toolchain_name}"),
            "check".to_owned(),
            "--release".to_owned(),
            "--locked".to_owned(),
            "--no-default-features".to_owned(),
            "--features".to_owned(),
            features.join(","),
        ],
    )
    .with_path_prefix(linker_bin)
}

pub fn cargo_build(
    package_root: &Path,
    toolchain_name: &str,
    linker_bin: &Path,
    features: &[String],
) -> ExecutedCommand {
    ExecutedCommand::new(
        "build-full-feature-artifact",
        package_root,
        "cargo",
        [
            format!("+{toolchain_name}"),
            "build".to_owned(),
            "--release".to_owned(),
            "--locked".to_owned(),
            "--no-default-features".to_owned(),
            "--features".to_owned(),
            features.join(","),
        ],
    )
    .with_path_prefix(linker_bin)
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
        let linker = Path::new("/sysroot/xtensa/bin");
        for command in [
            cargo_check(root, "esp-conduit-1.91.1", linker, &features),
            cargo_build(root, "esp-conduit-1.91.1", linker, &features),
        ] {
            assert_eq!(command.program, "cargo");
            assert_eq!(command.args[0], "+esp-conduit-1.91.1");
            assert!(!command.args.iter().any(|arg| arg == "+esp"));
            assert!(command.args.contains(&"--locked".into()));
            assert!(command.args.contains(&"--no-default-features".into()));
            assert_eq!(command.args.last().unwrap(), "bluetooth,kernel-signal");
            assert!(!command
                .args
                .iter()
                .any(|arg| arg == "--jobs" || arg == "-j"));
            assert_eq!(
                command.environment,
                [EnvironmentOverlay {
                    variable: "PATH".into(),
                    operation: "prepend".into(),
                    value: "/sysroot/xtensa/bin".into(),
                }]
            );
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

    #[test]
    fn compiler_observation_uses_only_the_named_toolchain() {
        let command = rustc_version(Path::new("/repo"), "esp-conduit-1.91.1");
        assert_eq!(command.program, "rustc");
        assert_eq!(command.args, ["+esp-conduit-1.91.1", "-Vv"]);
        assert!(command.environment.is_empty());
        let sysroot = rustc_sysroot(Path::new("/repo"), "esp-conduit-1.91.1");
        assert_eq!(sysroot.args, ["+esp-conduit-1.91.1", "--print", "sysroot"]);
        assert!(sysroot.environment.is_empty());
    }
}
