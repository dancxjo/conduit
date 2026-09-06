//! Minimal compiler boundary for the repository-owned CI impact planner.
//!
//! `cargo xtask ci` dispatches here so planning and attestation do not compile
//! unrelated hardware, product, and proof orchestration. The planner and shard
//! metadata remain the exact source files used by the full xtask binary.

#[cfg(feature = "host-release")]
use std::path::PathBuf;

#[cfg(feature = "host-release")]
#[path = "../../xtask/src/commands/host_release.rs"]
mod host_release;

mod proof {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProofClass {
        ContractCompile,
        LiveBrowser,
    }
}

mod process {
    use crate::proof::ProofClass;

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
    pub use crate::suite_network_capability as network_capability;
    pub use crate::suite_pico_compositions as pico_compositions;
    pub use crate::suite_workspace_shards as workspace_shards;
}

#[path = "../../xtask/src/commands/ci/impact.rs"]
mod impact;
#[path = "../../xtask/src/commands/ci/integration.rs"]
mod integration;
#[path = "../../xtask/src/commands/ci/monitor.rs"]
mod monitor;
#[path = "../../xtask/src/commands/ci/product_reconciliation.rs"]
mod product_reconciliation;
#[path = "../../xtask/src/commands/ci/promotion_snapshot.rs"]
mod promotion_snapshot;
#[path = "../../xtask/src/commands/ci/proof_graph.rs"]
mod proof_graph;
#[path = "../../xtask/src/commands/ci/rust_toolchain.rs"]
mod rust_toolchain;
#[path = "../../xtask/src/commands/ci/standalone_locks.rs"]
mod standalone_locks;
#[path = "../../xtask/src/workspace.rs"]
mod workspace;

mod ci_dispatch;
mod local_storage;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("host")
        && arguments.get(1).map(String::as_str) == Some("release")
    {
        #[cfg(feature = "host-release")]
        {
            // The isolated directory protects the running bootstrap executable;
            // it is not part of the Host artifact fabrication contract.
            std::env::remove_var("CARGO_TARGET_DIR");
            if let Err(error) = run_host_release(&arguments) {
                eprintln!("xtask error: {error}");
                std::process::exit(1);
            }
            return;
        }
        #[cfg(not(feature = "host-release"))]
        launch_host_release(&arguments);
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

#[cfg(not(feature = "host-release"))]
fn launch_host_release(arguments: &[String]) -> ! {
    // Windows cannot replace the dispatcher executable while it is running.
    // Compile the feature-bearing Host release binary in an isolated target.
    let status = std::process::Command::new("cargo")
        .env("CARGO_TARGET_DIR", "target/xtask-host-release")
        .args([
            "run",
            "--locked",
            "--package",
            "conduit-xtask-dispatch",
            "--features",
            "host-release",
            "--",
        ])
        .args(arguments)
        .status();
    match status {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("xtask error: cannot launch Host release dispatcher: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "host-release")]
fn run_host_release(arguments: &[String]) -> Result<(), String> {
    let mut values = arguments.iter().skip(2);
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
mod dependency_boundary_tests {
    use std::collections::BTreeSet;

    #[test]
    fn default_planner_test_target_has_only_dependency_light_inputs() {
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
        assert!(dependencies["conduit-host-browser-fabrication"]["optional"]
            .as_bool()
            .unwrap());
    }
}
