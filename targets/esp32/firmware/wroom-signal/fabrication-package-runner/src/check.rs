use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use conduit_host_esp32_fabrication::Esp32FabricationPackage;
use conduit_host_fabrication::{FabricationContribution, HostFabricationPackage};
use serde::Serialize;

use crate::{
    cli::Args,
    descriptor::{FabricationPackageDescriptor, PACKAGE_RELATIVE_PATH},
    process::{
        cargo_build, cargo_check, cargo_fmt, cargo_tree, linker_version, run_capture,
        rustc_sysroot, rustc_version, ExecutedCommand,
    },
    provenance::{self, InputProvenance},
    receipt::{CheckReceipt, EXCLUDED_TRUTH},
    selection::{checked_feature_projection, FeatureProjection},
};

const CONFIG_RELATIVE_PATH: &str = "targets/esp32/firmware/wroom-signal/.cargo/config.toml";
const LOCK_RELATIVE_PATH: &str = "targets/esp32/firmware/wroom-signal/Cargo.lock";
const FIRMWARE_MANIFEST_RELATIVE_PATH: &str = "targets/esp32/firmware/wroom-signal/Cargo.toml";
const RUNNER_MANIFEST_RELATIVE_PATH: &str =
    "targets/esp32/firmware/wroom-signal/fabrication-package-runner/Cargo.toml";
const EXPECTED_ESP_RELEASE: &str = "1.91.1-nightly";

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let package_root = args.repo_root.join(PACKAGE_RELATIVE_PATH);
    let projection = checked_feature_projection()?;
    let (descriptor, descriptor_bytes) = FabricationPackageDescriptor::read(&args.repo_root)?;
    descriptor.validate(&projection)?;
    validate_common_fabrication_package(&descriptor, &projection)?;

    let mut commands = Vec::new();
    let inputs = provenance::inspect(&args.repo_root, args.allow_dirty, &mut commands)?;
    let observed_toolchain =
        observe_toolchain(&args.repo_root, &descriptor.toolchain_name, &mut commands)?;
    validate_observed_toolchain(&observed_toolchain)?;
    let linker = observe_linker(&args.repo_root, &descriptor, &mut commands)?;
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
            cargo_check(
                &package_root,
                &descriptor.toolchain_name,
                &linker.bin,
                &projection.minimal_features,
            ),
            &mut commands,
        )?;
        run_capture(
            cargo_build(
                &package_root,
                &descriptor.toolchain_name,
                &linker.bin,
                &projection.full_features,
            ),
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
    let executed_commands_sha256 = provenance::sha256(&serde_json::to_vec(&commands)?);
    let check_identity = check_identity(&IdentityMaterial {
        source_sha: &inputs.source_sha,
        tracked_inputs_sha256: &inputs.tracked_inputs_sha256,
        lock_sha256: &lock_sha256,
        descriptor_sha256: &descriptor_sha256,
        config_sha256: &config_sha256,
        toolchain_sha256: &toolchain_sha256,
        toolchain_sysroot_sha256: &linker.sysroot_sha256,
        linker_sha256: &linker.identity_sha256,
        executed_commands_sha256: &executed_commands_sha256,
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
        linker,
        lock_sha256,
        descriptor_sha256,
        config_sha256,
        artifact_sha256,
        commands,
        executed_commands_sha256,
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
            "ESP32 FABRICATION PACKAGE CHECKED: {}",
            args.receipt.display()
        );
    }
    Ok(())
}

fn validate_common_fabrication_package(
    descriptor: &FabricationPackageDescriptor,
    projection: &FeatureProjection,
) -> Result<(), Box<dyn std::error::Error>> {
    let FabricationContribution::Anchor(package) = Esp32FabricationPackage.contribution() else {
        return Err("ESP32 package did not contribute its target anchor".into());
    };
    let target = package
        .targets
        .iter()
        .find(|target| target.key() == "esp32/xtensa-lx6/hw-463-esp-wroom-32")
        .ok_or("ESP32 fabrication package omitted its exact target")?;
    if package.package_id != "conduit-host-esp32@1"
        || package.package_revision != descriptor.revision
        || target.builder_adapter != descriptor.builder_adapter
        || target.toolchain_identity != descriptor.toolchain
    {
        return Err("common ESP32 fabrication-package identity drifted from its descriptor".into());
    }
    let features = |bases: &[conduit_host_fabrication::BaseSelection]| {
        let mut selected = bases
            .iter()
            .map(|base| {
                package
                    .offers
                    .iter()
                    .find(|offer| offer.implementation_id == base.driver)
                    .and_then(|offer| offer.build_feature.clone())
                    .ok_or_else(|| format!("ESP32 package omitted implementation {}", base.driver))
            })
            .collect::<Result<Vec<_>, _>>()?;
        selected.sort();
        selected.dedup();
        Ok::<_, String>(selected)
    };
    let minimal = features(&projection.minimal_bases)?;
    let full = features(&projection.full_bases)?;
    if minimal != descriptor.minimal_features || full != descriptor.full_features {
        return Err("common ESP32 fabrication-package feature projection drifted".into());
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
            "check-fabrication-runner-formatting",
        ),
    ] {
        run_capture(command, commands)?;
    }
    Ok(())
}

fn observe_toolchain(
    repo_root: &Path,
    toolchain_name: &str,
    commands: &mut Vec<ExecutedCommand>,
) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = run_capture(rustc_version(repo_root, toolchain_name), commands)?;
    Ok(String::from_utf8(bytes)?.trim().to_owned())
}

fn validate_observed_toolchain(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let release = text
        .lines()
        .find_map(|line| line.strip_prefix("release: "))
        .ok_or("named ESP rustc omitted its release identity")?;
    if release != EXPECTED_ESP_RELEASE {
        return Err(format!(
            "ESP Rust toolchain mismatch: expected {EXPECTED_ESP_RELEASE}, observed {release}"
        )
        .into());
    }
    if !text.lines().any(|line| line.starts_with("commit-hash: ")) {
        return Err("named ESP rustc omitted its commit hash".into());
    }
    Ok(())
}

#[derive(Debug)]
struct LinkerObservation {
    sysroot: String,
    sysroot_sha256: String,
    bin: PathBuf,
    identity: String,
    identity_sha256: String,
}

fn observe_linker(
    repo_root: &Path,
    descriptor: &FabricationPackageDescriptor,
    commands: &mut Vec<ExecutedCommand>,
) -> Result<LinkerObservation, Box<dyn std::error::Error>> {
    let sysroot = String::from_utf8(run_capture(
        rustc_sysroot(repo_root, &descriptor.toolchain_name),
        commands,
    )?)?
    .trim()
    .to_owned();
    let sysroot_path = PathBuf::from(&sysroot);
    if !sysroot_path.is_absolute() || !sysroot_path.is_dir() {
        return Err("named ESP Rust sysroot must be an existing absolute directory".into());
    }
    let bin = sysroot_path.join(&descriptor.linker_adapter);
    if !bin.is_dir() {
        return Err(format!("ESP linker adapter directory is absent: {}", bin.display()).into());
    }
    let linker = bin.join(&descriptor.linker_command);
    if !linker.is_file() {
        return Err(format!("ESP linker command is absent: {}", linker.display()).into());
    }
    let identity = String::from_utf8(run_capture(linker_version(repo_root, &linker), commands)?)?
        .trim()
        .to_owned();
    validate_observed_linker(&identity)?;
    Ok(LinkerObservation {
        sysroot_sha256: provenance::sha256(sysroot.as_bytes()),
        sysroot,
        bin,
        identity_sha256: provenance::sha256(identity.as_bytes()),
        identity,
    })
}

fn validate_observed_linker(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let first = text
        .lines()
        .next()
        .ok_or("ESP linker omitted its identity")?;
    if first != "xtensa-esp-elf-gcc (crosstool-NG esp-15.2.0_20250920) 15.2.0" {
        return Err(format!("ESP linker identity mismatch: observed `{first}`").into());
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
    toolchain_sysroot_sha256: &'a str,
    linker_sha256: &'a str,
    executed_commands_sha256: &'a str,
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
    descriptor: FabricationPackageDescriptor,
    projection: FeatureProjection,
    minimal_packages: BTreeSet<String>,
    full_packages: BTreeSet<String>,
    observed_toolchain: String,
    observed_toolchain_sha256: String,
    linker: LinkerObservation,
    lock_sha256: String,
    fabrication_descriptor_sha256: String,
    cargo_config_sha256: String,
    artifact_sha256: Option<String>,
    executed_commands: Vec<ExecutedCommand>,
    executed_commands_sha256: String,
    check_identity: String,
) -> CheckReceipt {
    CheckReceipt {
        schema: "conduit.host/fabrication-package-check@1",
        outcome: if dry_run { "planned" } else { "compiled" },
        proof_class: "machine-only-contract-compile",
        source_sha: inputs.source_sha,
        input_state: inputs.input_state,
        dirty_status_sha256: inputs.dirty_status_sha256,
        tracked_input_count: inputs.tracked_input_count,
        tracked_inputs_sha256: inputs.tracked_inputs_sha256,
        cargo_build_jobs: std::env::var("CARGO_BUILD_JOBS").ok(),
        lock_sha256,
        fabrication_descriptor_sha256,
        cargo_config_sha256,
        fabrication_package: descriptor.schema,
        fabrication_revision: descriptor.revision,
        builder_adapter: descriptor.builder_adapter,
        declared_toolchain: descriptor.toolchain,
        toolchain_name: descriptor.toolchain_name,
        observed_toolchain,
        observed_toolchain_sha256,
        toolchain_sysroot: linker.sysroot,
        toolchain_sysroot_sha256: linker.sysroot_sha256,
        linker_adapter: descriptor.linker_adapter,
        linker_command: descriptor.linker_command,
        linker_bin: linker.bin.display().to_string(),
        observed_linker: linker.identity,
        observed_linker_sha256: linker.identity_sha256,
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
        executed_commands_sha256,
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
    fn descriptor_is_bound_to_fabrication_package() {
        let projection = checked_feature_projection().unwrap();
        let descriptor = FabricationPackageDescriptor {
            schema: "conduit.host/fabrication-package/esp32-firmware@1".into(),
            package: "conduit-esp32-wroom-signal".into(),
            revision: 1,
            chip: "esp32".into(),
            board_descriptor: "observed/hw-463-esp-wroom-32@1".into(),
            target: "xtensa-esp32-none-elf".into(),
            toolchain: "esp-rs/rust-build@v1.91.1.0".into(),
            toolchain_name: "esp-conduit-1.91.1".into(),
            toolchain_action: "esp-rs/xtensa-toolchain@ec6d36527049a7f4fb2cb0c1a644668c1bb8a2a4"
                .into(),
            linker_adapter: "xtensa-esp-elf/esp-15.2.0_20250920/xtensa-esp-elf/bin".into(),
            linker_command: "xtensa-esp32-elf-gcc".into(),
            builder_adapter: "conduit-host-esp32/build-image@1".into(),
            minimal_features: projection.minimal_features.clone(),
            full_features: projection.full_features.clone(),
            minimal_bases: vec!["kernel-signal".into()],
            full_bases: vec!["kernel-signal".into(), "bluetooth-le-gatt".into()],
            artifact: "target/xtensa-esp32-none-elf/release/conduit-esp32-wroom-signal".into(),
        };
        validate_common_fabrication_package(&descriptor, &projection).unwrap();
    }

    #[test]
    fn exact_linker_identity_is_required() {
        validate_observed_linker(
            "xtensa-esp-elf-gcc (crosstool-NG esp-15.2.0_20250920) 15.2.0\nCopyright",
        )
        .unwrap();
        assert!(validate_observed_linker("xtensa-esp32-elf-gcc 15.2.0").is_err());
    }
}
