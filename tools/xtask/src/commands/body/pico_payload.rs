//! Exact authored Pico environmental payload production; no Body bootstrap grant.
use conduit_host_make::{
    check_host_configuration, parse_host_configuration_conduit, CheckedHostConfiguration,
    ConfigurationDevice, HostImage,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    path::Path,
    process::{Command, Stdio},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const MANIFEST: &str = "targets/rp2040/proof/environment-usb/Cargo.toml";
const BINARY: &str = "conduit-pico-environment-usb";
const TARGET: &str = "thumbv6m-none-eabi";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Recipe {
    pub configuration_id: String,
    pub target: String,
    pub features: Vec<&'static str>,
    pub binding_digest: String,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    BodyBootstrapUnavailable,
}
impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BodyBootstrapUnavailable: the exact environmental runtime does not yet consume and verify the canonical Body Spore provision")
    }
}
impl Error for Refusal {}

pub(crate) fn recipe(checked: &CheckedHostConfiguration) -> Result<Option<Recipe>> {
    let c = checked.configuration();
    if c.target.architecture != "thumbv6m"
        || !matches!(c.target.machine.as_str(), "pico" | "pico-w")
        || c.devices.is_empty()
    {
        return Ok(None);
    }
    // The package-owned checker/lowerer owns all supported peripheral shapes.
    let bindings = conduit_host_rp2040::environment_usb_rust(checked)
        .map_err(|why| format!("PicoPayloadProfileRefused: {why}"))?;
    let mut features = Vec::new();
    if c.target.machine == "pico-w" {
        features.push("wifi");
    }
    let has = |wanted: &str| {
        c.devices.iter().any(|device| match device {
            ConfigurationDevice::I2c { implementation, .. }
            | ConfigurationDevice::Gpio { implementation, .. }
            | ConfigurationDevice::Serial { implementation, .. }
            | ConfigurationDevice::Network { implementation, .. } => implementation == wanted,
        })
    };
    if has("dht11@1") {
        features.push("dht11-tm1637");
    } else if has("tm1637@1") {
        features.push("bosch-tm1637");
    }
    Ok(Some(Recipe {
        configuration_id: checked.configuration_id().into(),
        target: checked.profile().target.key(),
        features,
        binding_digest: sha(bindings.as_bytes()),
    }))
}

pub(crate) fn require_body_bootstrap(_recipe: &Recipe) -> std::result::Result<(), Refusal> {
    // Having real firmware and an invitation sector does not establish that
    // this executable will verify it or return its own admitted advertisement.
    Err(Refusal::BodyBootstrapUnavailable)
}

/// Uses the existing make-host-build entrance, with no signal template fallback.
pub(crate) fn try_build(
    source_path: &Path,
    image: &HostImage,
    description: &[u8],
    output: &Path,
    dry_run: bool,
) -> Result<bool> {
    if !matches!(
        image.manifest.target.as_str(),
        "conduitos/thumbv6m/pico" | "conduitos/thumbv6m/pico-w"
    ) {
        return Ok(false);
    }
    let source = fs::read_to_string(source_path)?;
    let configuration = parse_host_configuration_conduit(&source)
        .map_err(|why| format!("PicoPayloadSourceRefused: {why:?}"))?;
    let checked = check_host_configuration(
        configuration,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .map_err(|why| format!("PicoPayloadProfileRefused: {why:?}"))?;
    let Some(recipe) = recipe(&checked)? else {
        return Ok(false);
    };
    if image.payload.source_configuration_id.as_deref() != Some(recipe.configuration_id.as_str()) {
        return Err(
            "PicoPayloadSourceChanged: configuration differs from the checked IMAGE".into(),
        );
    }
    if dry_run {
        return Ok(true);
    }
    if output.exists() {
        return Err("PicoPayloadExists: refusing to overwrite earlier artifacts".into());
    }
    let toolchain=image.manifest.toolchain_identity.strip_prefix("rustc:")
        .and_then(|identity|identity.strip_suffix("+thumbv6m-none-eabi"))
        .ok_or("PicoPayloadToolchainMismatch: checked package did not select its exact RP2040 toolchain")?;
    // Refuse missing tools before paying for a cross build. Neither operation
    // acquires USB nor flashes, provisions credentials, or starts a Host.
    success(
        Command::new("picotool").arg("version"),
        "PicoPayloadToolUnavailable: picotool is required",
    )?;
    success(
        Command::new("rustup").args(["run", toolchain, "rustc", "--version"]),
        "PicoPayloadToolchainUnavailable: the selected RP2040 toolchain is required",
    )?;
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    let output = output.canonicalize()?;
    let frozen = output.join("input.host.conduit");
    fs::write(&frozen, source.as_bytes())?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let cache = root.join("target/pico-environment-payload");
    let mut cargo = Command::new("cargo");
    cargo
        .arg(format!("+{toolchain}"))
        .args([
            "build",
            "--locked",
            "--release",
            "--no-default-features",
            "--manifest-path",
        ])
        .arg(root.join(MANIFEST))
        .args(["--target", TARGET]);
    if !recipe.features.is_empty() {
        cargo.arg("--features").arg(recipe.features.join(","));
    }
    cargo
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", &cache)
        .env("CONDUIT_HOST_CONFIGURATION", &frozen);
    success(
        &mut cargo,
        "PicoPayloadBuildFailed: exact environmental runtime refused",
    )?;
    let elf = fs::read(cache.join(TARGET).join("release").join(BINARY))?;
    if elf.len() > 32 * 1024 * 1024
        || !elf.starts_with(b"\x7fELF")
        || !elf
            .windows(recipe.configuration_id.len())
            .any(|bytes| bytes == recipe.configuration_id.as_bytes())
    {
        return Err(
            "PicoPayloadIdentityMismatch: ELF does not retain the checked configuration".into(),
        );
    }
    let elf_path = output.join("firmware.elf");
    fs::write(&elf_path, &elf)?;
    let uf2_path = output.join("firmware.uf2");
    success(
        Command::new("picotool")
            .args(["uf2", "convert"])
            .arg(&elf_path)
            .arg(&uf2_path)
            .args(["--family", "rp2040"]),
        "PicoPayloadConversionFailed: official UF2 conversion refused",
    )?;
    // Reuse the frozen browser/target flash-layout parser, including the
    // credentials/bootstrap exclusion, instead of maintaining a second parser.
    success(
        Command::new("node")
            .arg(root.join("targets/rp2040/deployment/build/verify-payload.mjs"))
            .arg(&uf2_path),
        "PicoPayloadLayoutRefused: UF2 does not preserve reserved regions",
    )?;
    let uf2 = fs::read(uf2_path)?;
    fs::write(output.join("resolved-image.json"), description)?;
    let receipt = serde_json::json!({"schema":"conduit.host/pico-environment-payload@1","recipe":recipe,"manifest":MANIFEST,"binary":BINARY,"toolchain":toolchain,"rust_target":TARGET,"source_identity":image.manifest.source_identity,"elf_sha256":sha(&elf),"uf2_sha256":sha(&uf2),"host_source_sha256":sha(source.as_bytes()),"body_spore_created":false,"body_bootstrap":"unavailable","host_boot_observed":false,"membership_admitted":false,"flashed":false});
    fs::write(
        output.join("payload-receipt.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(true)
}
fn success(command: &mut Command, label: &str) -> Result<()> {
    let status = command
        .stdout(Stdio::null())
        .status()
        .map_err(|error| format!("{label}: {error}"))?;
    if !status.success() {
        return Err(label.into());
    }
    Ok(())
}
fn sha(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checked(source: &str) -> CheckedHostConfiguration {
        check_host_configuration(
            parse_host_configuration_conduit(source).unwrap(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
        )
        .unwrap()
    }
    #[test]
    fn real_peripheral_profiles_select_their_exact_runtime_features() {
        for (source, features) in [
            (
                include_str!(
                    "../../../../../targets/rp2040/profiles/pico-w-environment-usb.host.conduit"
                ),
                vec!["wifi"],
            ),
            (
                include_str!(
                    "../../../../../targets/rp2040/profiles/pico-w-bme280-tm1637.host.conduit"
                ),
                vec!["wifi", "bosch-tm1637"],
            ),
            (
                include_str!(
                    "../../../../../targets/rp2040/profiles/pico-w-dht11-tm1637.host.conduit"
                ),
                vec!["wifi", "dht11-tm1637"],
            ),
            (
                include_str!(
                    "../../../../../targets/rp2040/profiles/pico-environment-usb.host.conduit"
                ),
                vec![],
            ),
        ] {
            let recipe = recipe(&checked(source)).unwrap().unwrap();
            assert_eq!(recipe.features, features);
            assert_eq!(
                require_body_bootstrap(&recipe),
                Err(Refusal::BodyBootstrapUnavailable)
            );
        }
    }
    #[test]
    fn unsupported_attachments_do_not_fall_back_to_an_unrelated_template() {
        let source =
            include_str!("../../../../../targets/rp2040/profiles/pico-w-dht11-tm1637.host.conduit");
        let invalid = source.replace("half_period_us: 50", "half_period_us: 0");
        assert!(recipe(&checked(&invalid)).is_err());
        let serial_only = checked(include_str!(
            "../../../../../targets/rp2040/profiles/pico-w.host.conduit"
        ));
        assert!(recipe(&serial_only).unwrap().is_none());
    }
}
