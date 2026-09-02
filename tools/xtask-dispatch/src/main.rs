//! Minimal compiler boundary for the repository-owned CI impact planner.
//!
//! `cargo xtask ci plan` dispatches here so planning does not compile unrelated
//! hardware, product, and proof orchestration. The planner and shard metadata
//! remain the exact source files used by the full xtask binary.

use std::path::PathBuf;

#[path = "../../../xtask/src/commands/host_release.rs"]
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

#[path = "../../../xtask/src/suites/check.rs"]
pub mod suite_check;
#[path = "../../../xtask/src/suites/network_capability.rs"]
pub mod suite_network_capability;
#[path = "../../../xtask/src/suites/pico_compositions.rs"]
pub mod suite_pico_compositions;
#[path = "../../../xtask/src/suites/workspace_shards.rs"]
pub mod suite_workspace_shards;

mod suites {
    pub use crate::suite_check as check;
    pub use crate::suite_network_capability as network_capability;
    pub use crate::suite_pico_compositions as pico_compositions;
    pub use crate::suite_workspace_shards as workspace_shards;
}

#[path = "../../../xtask/src/commands/ci/impact.rs"]
mod impact;
#[path = "../../../xtask/src/workspace.rs"]
mod workspace;

struct Arguments {
    base: String,
    head: String,
    json_out: Option<PathBuf>,
    summary_out: Option<PathBuf>,
}

fn parse(arguments: &[String]) -> Result<Arguments, String> {
    let mut values = arguments.iter().skip(2);
    let base = values
        .next()
        .cloned()
        .ok_or_else(|| "missing base commit".to_owned())?;
    let head = values
        .next()
        .cloned()
        .ok_or_else(|| "missing head commit".to_owned())?;
    let mut json_out = None;
    let mut summary_out = None;
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--locked" => {}
            "--json-out" => {
                json_out = Some(PathBuf::from(
                    values
                        .next()
                        .ok_or_else(|| "missing --json-out path".to_owned())?,
                ));
            }
            "--summary-out" => {
                summary_out = Some(PathBuf::from(
                    values
                        .next()
                        .ok_or_else(|| "missing --summary-out path".to_owned())?,
                ));
            }
            other => return Err(format!("unsupported ci plan argument: {other}")),
        }
    }
    Ok(Arguments {
        base,
        head,
        json_out,
        summary_out,
    })
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("host")
        && arguments.get(1).map(String::as_str) == Some("release")
    {
        if let Err(error) = run_host_release(&arguments) {
            eprintln!("xtask error: {error}");
            std::process::exit(1);
        }
        return;
    }
    if arguments.first().map(String::as_str) != Some("ci")
        || arguments.get(1).map(String::as_str) != Some("plan")
    {
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

    let result = parse(&arguments).and_then(|args| {
        impact::run(
            &args.base,
            &args.head,
            args.json_out.as_deref(),
            args.summary_out.as_deref(),
        )
        .map_err(|error| error.to_string())
    });
    if let Err(error) = result {
        eprintln!("xtask error: {error}");
        std::process::exit(1);
    }
}

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
