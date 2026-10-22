//! Package ownership alone does not preserve feature-gated test coverage.
use super::WorkspaceShard;
use std::{
    collections::{BTreeMap, BTreeSet},
    process::Command,
};

fn features(arguments: &[&str]) -> BTreeMap<String, BTreeSet<String>> {
    let output = Command::new("cargo")
        .args([
            "tree",
            "--color",
            "never",
            "--locked",
            "--edges",
            "normal,build,dev",
            "--prefix",
            "none",
            "--format",
            "{p}|{f}",
        ])
        .args(arguments)
        // Exercise the same forced-colour parent environment as Actions.
        // The explicit command option keeps the feature inventory plain.
        .env("CARGO_TERM_COLOR", "always")
        .output()
        .expect("Cargo feature inventory launches");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut result: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        let Some((package, enabled)) = line.split_once('|') else {
            continue;
        };
        let name = package.split_whitespace().next().unwrap();
        result.entry(name.into()).or_default().extend(
            enabled
                .trim_end_matches(" (*)")
                .split(',')
                .filter(|feature| !feature.is_empty())
                .map(str::to_owned),
        );
    }
    result
}

#[test]
fn parallel_test_roots_retain_workspace_enabled_features() {
    let workspace = features(&["--workspace"]);
    for shard in [
        WorkspaceShard::TestFoundation,
        WorkspaceShard::TestHostsStd,
        WorkspaceShard::TestHostsBrowser,
        WorkspaceShard::TestHostsConduitos,
        WorkspaceShard::TestHostsWorkbench,
        WorkspaceShard::TestProducts,
    ] {
        let step = shard.package_test_step().unwrap();
        let mut arguments = Vec::new();
        let mut packages = Vec::new();
        for pair in step.args.windows(2) {
            if pair[0] == "-p" {
                arguments.extend_from_slice(pair);
                packages.push(pair[1]);
            } else if pair[0] == "--features" {
                arguments.extend_from_slice(pair);
            }
        }
        let selected = features(&arguments);
        for package in packages {
            let expected = workspace.get(package).expect("workspace package exists");
            let actual = selected.get(package).expect("shard package exists");
            let missing: Vec<_> = expected.difference(actual).collect();
            assert!(
                missing.is_empty(),
                "{} loses {package} features: {missing:?}",
                step.id
            );
        }
    }
}

#[test]
fn delegated_conduitos_retains_workspace_enabled_features() {
    let workspace = features(&["--workspace"]);
    let arguments = crate::suites::host_protocol_shards::arguments();
    let arguments: Vec<_> = arguments
        .iter()
        .skip(1)
        .map(String::as_str)
        .filter(|arg| !matches!(*arg, "--locked" | "--no-fail-fast"))
        .collect();
    let selected = features(&arguments);
    let package = crate::suites::host_protocol_shards::PACKAGE;
    let missing: Vec<_> = workspace[package].difference(&selected[package]).collect();
    assert!(
        missing.is_empty(),
        "delegated {package} loses features: {missing:?}"
    );
}

#[test]
fn native_model_backing_is_optional_and_std_ci_exercises_api23_without_downloads() {
    let default = features(&["-p", "conduit-std-host", "--no-default-features"]);
    assert!(!default.contains_key("ort-sys"));
    let step = WorkspaceShard::TestHostsStd.package_test_step().unwrap();
    let feature_argument = step
        .args
        .windows(2)
        .find(|pair| pair[0] == "--features")
        .unwrap()[1];
    assert!(feature_argument
        .split(',')
        .any(|f| f == "conduit-std-host/native-model-backing"));
    let selected = features(&[
        "-p",
        "conduit-std-host",
        "--features",
        "native-model-backing",
    ]);
    let native = &selected["ort-sys"];
    assert!(native.contains("api-23") && native.contains("disable-linking"));
    assert!(!native.contains("download-binaries") && !native.contains("api-24"));
}
