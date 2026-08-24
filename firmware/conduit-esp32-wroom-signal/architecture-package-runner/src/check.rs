use std::{collections::BTreeSet, fs, path::Path};

use serde::Serialize;

use crate::{
    cli::Args,
    descriptor::{ArchitecturePackageDescriptor, PACKAGE_RELATIVE_PATH},
    process::{cargo_build, cargo_check, cargo_fmt, cargo_tree, run_capture, ExecutedCommand},
    provenance::{self, InputProvenance},
    receipt::{CheckReceipt, EXCLUDED_TRUTH},
    selection::{checked_feature_projection, FeatureProjection},
};

const CONFIG_RELATIVE_PATH: &str = "firmware/conduit-esp32-wroom-signal/.cargo/config.toml";
const LOCK_RELATIVE_PATH: &str = "firmware/conduit-esp32-wroom-signal/Cargo.lock";
const FIRMWARE_MANIFEST_RELATIVE_PATH: &str = "firmware/conduit-esp32-wroom-signal/Cargo.toml";
const RUNNER_MANIFEST_RELATIVE_PATH: &str =
    "firmware/conduit-esp32-wroom-signal/architecture-package-runner/Cargo.toml";
const EXPECTED_ESP_RELEASE: &str = "1.91.1-nightly";

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let package_root = args.repo_root.join(PACKAGE_RELATIVE_PATH);
    let projection = checked_feature_projection()?;
    let (descriptor, descriptor_bytes) = ArchitecturePackageDescriptor::read(&args.repo_root)?;
    descriptor.validate(&projection)?;
    validate_common_architecture_package(&descriptor, &projection)?;

    let mut commands = Vec::new();
    let inputs = provenance::inspect(&args.repo_root, args.allow_dirty, &mut commands)?;
    let observed_toolchain = observe_toolchain(&args.repo_root, &mut commands)?;
    validate_observed_toolchain(&observed_toolchain)?;
    run_format_checks(&args.repo_root, &mut commands)?;

    let minimal_packages = runtime_packages(
        &package_root,
        &projection.minimal_features,
        "resolve-minimal-runtime-closure",
        &mut commands,
    )?;
    let full_packages = runtime_packages(
        &package_root,
        &projection.full_features,
        "resolve-full-runtime-closure",
        &mut commands,
    )?;
    validate_closure(&minimal_packages, &full_packages)?;

    let artifact_sha256 = if args.dry_run {
        None
    } else {
        run_capture(
            cargo_check(&package_root, &projection.minimal_features),
            &mut commands,
        )?;
        run_capture(
            cargo_build(&package_root, &projection.full_features),
            &mut commands,
        )?;
        Some(provenance::sha256(&fs::read(
            package_root.join(&descriptor.artifact),
        )?))
    };

    let lock_sha256 = provenance::sha256(&fs::read(args.repo_root.join(LOCK_RELATIVE_PATH))?);
    let descriptor_sha256 = provenance::sha256(&descriptor_bytes);
    let config_sha256 = provenance::sha256(&fs::read(args.repo_root.join(CONFIG_RELATIVE_PATH))?);
    let toolchain_sha256 = provenance::sha256(observed_toolchain.as_bytes());
    let check_identity = check_identity(&IdentityMaterial {
        source_sha: &inputs.source_sha,
        tracked_inputs_sha256: &inputs.tracked_inputs_sha256,
        lock_sha256: &lock_sha256,
        descriptor_sha256: &descriptor_sha256,
        config_sha256: &config_sha256,
        toolchain_sha256: &toolchain_sha256,
        minimal_features: &projection.minimal_features,
        full_features: &projection.full_features,
        artifact_sha256: artifact_sha256.as_deref(),
    })?;

    let receipt = make_receipt(
        args.dry_run,
        inputs,
        descriptor,
        projection,
        minimal_packages,
        full_packages,
        observed_toolchain,
        toolchain_sha256,
        lock_sha256,
        descriptor_sha256,
        config_sha256,
        artifact_sha256,
        commands,
        check_identity,
    );
    if let Some(parent) = args.receipt.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.receipt, serde_json::to_vec_pretty(&receipt)?)?;
    if args.json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else if !args.quiet {
        println!(
            "ESP32 ARCHITECTURE PACKAGE CHECKED: {}",
            args.receipt.display()
        );
    }
    Ok(())
}

fn validate_common_architecture_package(
    descriptor: &ArchitecturePackageDescriptor,
    projection: &FeatureProjection,
) -> Result<(), Box<dyn std::error::Error>> {
    let package = conduit_host_fabrication::architecture_packages()
        .iter()
        .find(|package| package.id == "esp32-firmware@1")
        .ok_or("common Host-fabrication catalog omitted `esp32-firmware@1`")?;
    if package.revision != 2
        || package.builder != descriptor.builder_adapter
        || package.toolchain != descriptor.toolchain
    {
        return Err(
            "common ESP32 architecture-package identity drifted from its descriptor".into(),
        );
    }
    let minimal = conduit_host_fabrication::derive_esp32_feature_closure(&projection.minimal_bases)
        .map_err(|diagnostic| {
            format!("common minimal feature derivation refused: {diagnostic:?}")
        })?;
    let full = conduit_host_fabrication::derive_esp32_feature_closure(&projection.full_bases)
        .map_err(|diagnostic| format!("common full feature derivation refused: {diagnostic:?}"))?;
    if minimal != descriptor.minimal_features || full != descriptor.full_features {
        return Err("common ESP32 architecture-package feature projection drifted".into());
    }
    Ok(())
}

fn run_format_checks(
    repo_root: &Path,
    commands: &mut Vec<ExecutedCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
    for command in [
        cargo_fmt(
            repo_root,
            &repo_root.join(FIRMWARE_MANIFEST_RELATIVE_PATH),
            "check-firmware-formatting",
        ),
        cargo_fmt(
            repo_root,
            &repo_root.join(RUNNER_MANIFEST_RELATIVE_PATH),
            "check-architecture-runner-formatting",
        ),
    ] {
        run_capture(command, commands)?;
    }
    Ok(())
}

fn observe_toolchain(
    repo_root: &Path,
    commands: &mut Vec<ExecutedCommand>,
) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = run_capture(
        ExecutedCommand::new(
            "observe-esp-rust-toolchain",
            repo_root,
            "rustc",
            ["+esp", "-Vv"],
        ),
        commands,
    )?;
    Ok(String::from_utf8(bytes)?.trim().to_owned())
}

fn validate_observed_toolchain(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let release = text
        .lines()
        .find_map(|line| line.strip_prefix("release: "))
        .ok_or("`rustc +esp -Vv` omitted its release identity")?;
    if release != EXPECTED_ESP_RELEASE {
        return Err(format!(
            "ESP Rust toolchain mismatch: expected {EXPECTED_ESP_RELEASE}, observed {release}"
        )
        .into());
    }
    if !text.lines().any(|line| line.starts_with("commit-hash: ")) {
        return Err("`rustc +esp -Vv` omitted its commit hash".into());
    }
    Ok(())
}

fn runtime_packages(
    package_root: &Path,
    features: &[String],
    purpose: &str,
    commands: &mut Vec<ExecutedCommand>,
) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let output = run_capture(cargo_tree(package_root, features, purpose), commands)?;
    Ok(String::from_utf8(output)?
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect())
}

fn validate_closure(
    minimal: &BTreeSet<String>,
    full: &BTreeSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !minimal.is_subset(full) || minimal == full {
        return Err("minimal ESP32 runtime closure must be a strict subset of full closure".into());
    }
    for optional in ["esp-radio", "trouble-host"] {
        if minimal.contains(optional) || !full.contains(optional) {
            return Err(format!("optional Base closure mapping refused for {optional}").into());
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct IdentityMaterial<'a> {
    source_sha: &'a str,
    tracked_inputs_sha256: &'a str,
    lock_sha256: &'a str,
    descriptor_sha256: &'a str,
    config_sha256: &'a str,
    toolchain_sha256: &'a str,
    minimal_features: &'a [String],
    full_features: &'a [String],
    artifact_sha256: Option<&'a str>,
}

fn check_identity(value: &IdentityMaterial<'_>) -> Result<String, Box<dyn std::error::Error>> {
    Ok(provenance::sha256(&serde_json::to_vec(value)?))
}

#[allow(clippy::too_many_arguments)]
fn make_receipt(
    dry_run: bool,
    inputs: InputProvenance,
    descriptor: ArchitecturePackageDescriptor,
    projection: FeatureProjection,
    minimal_packages: BTreeSet<String>,
    full_packages: BTreeSet<String>,
    observed_toolchain: String,
    observed_toolchain_sha256: String,
    lock_sha256: String,
    architecture_descriptor_sha256: String,
    cargo_config_sha256: String,
    artifact_sha256: Option<String>,
    executed_commands: Vec<ExecutedCommand>,
    check_identity: String,
) -> CheckReceipt {
    CheckReceipt {
        schema: "conduit.architecture-package/check-receipt@2",
        outcome: if dry_run { "planned" } else { "compiled" },
        proof_class: "machine-only-contract-compile",
        source_sha: inputs.source_sha,
        input_state: inputs.input_state,
        dirty_status_sha256: inputs.dirty_status_sha256,
        tracked_input_count: inputs.tracked_input_count,
        tracked_inputs_sha256: inputs.tracked_inputs_sha256,
        cargo_build_jobs: std::env::var("CARGO_BUILD_JOBS").ok(),
        lock_sha256,
        architecture_descriptor_sha256,
        cargo_config_sha256,
        architecture_package: descriptor.schema,
        architecture_revision: descriptor.revision,
        builder_adapter: descriptor.builder_adapter,
        declared_toolchain: descriptor.toolchain,
        observed_toolchain,
        observed_toolchain_sha256,
        target: descriptor.target,
        chip: descriptor.chip,
        board_descriptor: descriptor.board_descriptor,
        minimal_bases: projection.minimal_bases,
        full_bases: projection.full_bases,
        minimal_features: projection.minimal_features,
        full_features: projection.full_features,
        minimal_runtime_packages: minimal_packages.into_iter().collect(),
        full_runtime_packages: full_packages.into_iter().collect(),
        artifact_sha256,
        executed_commands,
        check_identity,
        excluded_truth: EXCLUDED_TRUTH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINNED: &str = concat!(
        "rustc 1.91.1-nightly (719630278 2025-08-24)\n",
        "binary: rustc\n",
        "commit-hash: 71963027884c9d995173aca298b0fe614e145bde\n",
        "commit-date: 2025-08-24\n",
        "host: x86_64-unknown-linux-gnu\n",
        "release: 1.91.1-nightly\n",
        "LLVM version: 20.1.8\n",
    );

    #[test]
    fn pinned_observed_toolchain_is_accepted() {
        validate_observed_toolchain(PINNED).unwrap();
    }

    #[test]
    fn local_or_incomplete_toolchain_identity_is_refused() {
        assert!(validate_observed_toolchain(&PINNED.replace("1.91.1", "1.95.0")).is_err());
        assert!(validate_observed_toolchain("release: 1.91.1-nightly").is_err());
    }

    #[test]
    fn closure_must_be_strict_and_keep_ble_optional() {
        let minimal = BTreeSet::from(["kernel".into()]);
        let full = BTreeSet::from(["kernel".into(), "esp-radio".into(), "trouble-host".into()]);
        validate_closure(&minimal, &full).unwrap();
        assert!(validate_closure(&full, &full).is_err());
    }

    #[test]
    fn descriptor_is_bound_to_common_architecture_catalog() {
        let projection = checked_feature_projection().unwrap();
        let descriptor = ArchitecturePackageDescriptor {
            schema: "conduit.architecture-package/esp32-firmware@1".into(),
            package: "conduit-esp32-wroom-signal".into(),
            revision: 2,
            chip: "esp32".into(),
            board_descriptor: "observed/hw-463-esp-wroom-32@1".into(),
            target: "xtensa-esp32-none-elf".into(),
            toolchain: "esp-rs/rust-build@v1.91.1.0".into(),
            toolchain_action: "esp-rs/xtensa-toolchain@ec6d36527049a7f4fb2cb0c1a644668c1bb8a2a4"
                .into(),
            builder_adapter: "esp32-firmware/architecture-package-runner@2".into(),
            minimal_features: projection.minimal_features.clone(),
            full_features: projection.full_features.clone(),
            minimal_bases: vec!["kernel-signal".into()],
            full_bases: vec!["kernel-signal".into(), "bluetooth-le-gatt".into()],
            artifact: "target/xtensa-esp32-none-elf/release/conduit-esp32-wroom-signal".into(),
        };
        validate_common_architecture_package(&descriptor, &projection).unwrap();
    }
}
