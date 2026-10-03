//! Dependency-light entrance for repository CI and build setup.
//!
//! `cargo xtask ci` dispatches here without compiling unrelated hardware and
//! product orchestration. Unit shard metadata is shared with the full xtask.

#[cfg(feature = "host-release")]
use std::path::PathBuf;

#[cfg(feature = "host-release")]
#[path = "../../xtask/src/commands/host_release.rs"]
mod host_release;

const HOST_RELEASE_BOOTSTRAP_ENV: &str = "CONDUIT_XTASK_HOST_RELEASE_BOOTSTRAP";
const HOST_RELEASE_REQUESTED_TARGET_ENV: &str = "CONDUIT_XTASK_HOST_RELEASE_REQUESTED_TARGET";

mod proof {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProofClass {
        ContractCompile,
        LiveBrowser,
    }
}

#[path = "../../xtask/src/process/tool_command.rs"]
mod tool_command;

mod process {
    use crate::proof::ProofClass;
    pub use crate::tool_command::command_for;

    #[derive(Debug, Clone)]
    pub struct Step {
        pub id: &'static str,
        pub description: &'static str,
        pub program: &'static str,
        pub args: &'static [&'static str],
        pub cwd: Option<&'static str>,
        pub tool_or_target: Option<&'static str>,
        pub proof_class: Option<ProofClass>,
        pub expected_artifacts: &'static [&'static str],
    }

    impl Step {
        pub const fn new(
            id: &'static str,
            description: &'static str,
            program: &'static str,
            args: &'static [&'static str],
        ) -> Self {
            Self {
                id,
                description,
                program,
                args,
                cwd: None,
                tool_or_target: None,
                proof_class: None,
                expected_artifacts: &[],
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub const fn typed(
            id: &'static str,
            description: &'static str,
            program: &'static str,
            args: &'static [&'static str],
            cwd: Option<&'static str>,
            tool_or_target: Option<&'static str>,
            proof_class: Option<ProofClass>,
            expected_artifacts: &'static [&'static str],
        ) -> Self {
            Self {
                id,
                description,
                program,
                args,
                cwd,
                tool_or_target,
                proof_class,
                expected_artifacts,
            }
        }
    }
}

#[path = "../../xtask/src/suites/check.rs"]
pub mod suite_check;
#[path = "../../xtask/src/suites/network_capability.rs"]
pub mod suite_network_capability;
#[path = "../../xtask/src/suites/pico_compositions.rs"]
pub mod suite_pico_compositions;
#[path = "../../xtask/src/suites/workspace_shards.rs"]
pub mod suite_workspace_shards;

mod suites {
    pub use crate::suite_check as check;
    #[cfg(test)]
    pub use crate::suite_network_capability as network_capability;
    #[cfg(test)]
    pub use crate::suite_pico_compositions as pico_compositions;
    pub use crate::suite_workspace_shards as workspace_shards;
}

#[path = "../../xtask/src/commands/ci/rust_toolchain.rs"]
mod rust_toolchain;
#[path = "../../xtask/src/commands/ci/standalone_locks.rs"]
mod standalone_locks;
#[path = "../../xtask/src/workspace.rs"]
mod workspace;

mod ci_dispatch;
mod local_storage;
mod tool_setup;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let Some(setup) = tool_setup::route(&arguments) {
        if let Err(error) = tool_setup::run(setup) {
            eprintln!("xtask error: {error}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(options) = host_release_options(&arguments) {
        #[cfg(feature = "host-release")]
        {
            // The bootstrap executable uses an isolated Cargo target. Restore
            // the caller's target directory for the actual Host product build.
            if std::env::var_os(HOST_RELEASE_BOOTSTRAP_ENV).is_some() {
                if let Some(target) = std::env::var_os(HOST_RELEASE_REQUESTED_TARGET_ENV) {
                    std::env::set_var("CARGO_TARGET_DIR", target);
                } else {
                    std::env::remove_var("CARGO_TARGET_DIR");
                }
                std::env::remove_var(HOST_RELEASE_BOOTSTRAP_ENV);
                std::env::remove_var(HOST_RELEASE_REQUESTED_TARGET_ENV);
            }
            if let Err(error) = run_host_release(options) {
                eprintln!("xtask error: {error}");
                std::process::exit(1);
            }
            return;
        }
        #[cfg(not(feature = "host-release"))]
        launch_host_release(options);
    }
    if arguments.first().map(String::as_str) != Some("ci") {
        let status = std::process::Command::new("cargo")
            .args(["run", "--package", "xtask", "--"])
            .args(&arguments)
            .status();
        match status {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(error) => {
                eprintln!("xtask error: cannot launch full xtask: {error}");
                std::process::exit(1);
            }
        }
    }

    let result = ci_dispatch::run(&arguments);
    if let Err(error) = result {
        eprintln!("xtask error: {error}");
        std::process::exit(1);
    }
}

fn host_release_options(arguments: &[String]) -> Option<&[String]> {
    match arguments {
        [host, release, options @ ..] if host == "host" && release == "release" => Some(options),
        [make, host, release, options @ ..]
            if make == "make" && host == "host" && release == "release" =>
        {
            Some(options)
        }
        _ => None,
    }
}

#[cfg(not(feature = "host-release"))]
fn launch_host_release(options: &[String]) -> ! {
    // Windows cannot replace the dispatcher executable while it is running.
    // Compile the feature-bearing Host release binary in an isolated target.
    let requested_target = std::env::var_os("CARGO_TARGET_DIR");
    let mut command = std::process::Command::new("cargo");
    command
        .env("CARGO_TARGET_DIR", "target/xtask-host-release")
        .env(HOST_RELEASE_BOOTSTRAP_ENV, "1")
        .args([
            "run",
            "--locked",
            "--package",
            "conduit-xtask-dispatch",
            "--features",
            "host-release",
            "--",
            "host",
            "release",
        ])
        .args(options);
    if let Some(target) = requested_target {
        command.env(HOST_RELEASE_REQUESTED_TARGET_ENV, target);
    } else {
        command.env_remove(HOST_RELEASE_REQUESTED_TARGET_ENV);
    }
    let status = command.status();
    match status {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("xtask error: cannot launch Host release dispatcher: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "host-release")]
fn run_host_release(options: &[String]) -> Result<(), String> {
    let mut values = options.iter();
    let mut output = None;
    let mut platform = None;
    let mut source_identity = None;
    let mut json = false;
    let mut quiet = false;
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--locked" => {}
            "--json" => json = true,
            "--quiet" => quiet = true,
            "--output" => {
                output = Some(PathBuf::from(values.next().ok_or("missing --output path")?));
            }
            "--platform" => {
                platform = Some(match values.next().map(String::as_str) {
                    Some("browser") => host_release::ReleasePlatform::Browser,
                    Some("linux") => host_release::ReleasePlatform::Linux,
                    Some("windows") => host_release::ReleasePlatform::Windows,
                    Some("macos") => host_release::ReleasePlatform::Macos,
                    Some(other) => return Err(format!("unsupported release platform: {other}")),
                    None => return Err("missing --platform value".into()),
                });
            }
            "--source-identity" => {
                source_identity = Some(
                    values
                        .next()
                        .cloned()
                        .ok_or("missing --source-identity value")?,
                );
            }
            other => return Err(format!("unsupported host release argument: {other}")),
        }
    }
    let output = output.ok_or("host release requires --output")?;
    let platform = platform.ok_or("host release requires --platform")?;
    let source_identity = match source_identity {
        Some(identity) => identity,
        None => command_identity("git", &["rev-parse", "HEAD"])?,
    };
    host_release::run(
        &output,
        platform,
        &source_identity,
        &host_release::ReleaseOptions { json, quiet },
    )
    .map_err(|error| error.to_string())
}

#[cfg(feature = "host-release")]
fn command_identity(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot execute {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot derive exact identity from {program}: {}",
            output.status
        ));
    }
    let identity = String::from_utf8(output.stdout)
        .map_err(|error| format!("{program} identity is not UTF-8: {error}"))?
        .trim()
        .to_owned();
    if identity.is_empty() {
        return Err(format!("{program} returned an empty identity"));
    }
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::host_release_options;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn host_release_dispatch_accepts_supported_make_entrance() {
        let arguments = arguments(&[
            "make",
            "host",
            "release",
            "--platform",
            "windows",
            "--output",
            "target/releases",
        ]);

        assert_eq!(host_release_options(&arguments), Some(&arguments[3..]));
    }

    #[test]
    fn host_release_dispatch_retains_internal_direct_entrance() {
        let arguments = arguments(&[
            "host",
            "release",
            "--platform",
            "macos",
            "--output",
            "target/releases",
        ]);

        assert_eq!(host_release_options(&arguments), Some(&arguments[2..]));
    }

    #[test]
    fn unrelated_make_commands_stay_on_the_full_dispatcher() {
        let arguments = arguments(&["make", "conduitos", "release"]);
        assert_eq!(host_release_options(&arguments), None);
    }
}

#[cfg(test)]
mod dependency_boundary_tests {
    use std::collections::BTreeSet;

    #[test]
    fn default_dispatcher_test_target_has_only_dependency_light_inputs() {
        let root = crate::workspace::workspace_root().unwrap();
        let manifest = std::fs::read_to_string(root.join("tools/xtask-dispatch/Cargo.toml"))
            .expect("read dispatcher manifest");
        let manifest: toml::Value = toml::from_str(&manifest).expect("parse dispatcher manifest");
        let dependencies = manifest["dependencies"]
            .as_table()
            .expect("dispatcher dependencies");
        let non_optional = dependencies
            .iter()
            .filter_map(|(name, value)| {
                let optional = value
                    .as_table()
                    .and_then(|details| details.get("optional"))
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false);
                (!optional).then_some(name.as_str())
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            non_optional,
            BTreeSet::from(["serde", "serde_json", "sha2", "toml"])
        );
        assert!(dependencies["conduit-host-browser-make"]["optional"]
            .as_bool()
            .unwrap());
    }
}
