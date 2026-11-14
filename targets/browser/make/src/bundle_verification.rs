//! Target-owned verification of the existing profile-gated BrowserBundle.
//! This checks the same build basis / IMAGE payload as browser-bundle.mjs.
//! Generic HostImage, rich browser IMAGE and final ZIP identity remain distinct.
use crate::BROWSER_IMPLEMENTATIONS;
use conduit_host_make::{
    canonical_profile_json, CheckedHostConfiguration, HostImage, HostReleaseManifest,
    SporeOutputKind, VerifiedHostReleaseBytes,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
mod model;
use model::*;
const TARGET: &str = "browser/wasm32/page";
const BOOT: &str = "browser-boot-profile.mjs";
const IMAGE: &str = "conduit-browser-image.json";
const DISTRIBUTION: &str = "conduit.browser/reviewed-distribution@1";

pub struct VerifiedBrowserBundleImage {
    release: VerifiedHostReleaseBytes,
    distribution: VerifiedHostReleaseBytes,
    generic_image_id: String,
    bundle_image_id: String,
    profile_id: String,
    configuration_id: String,
}
impl VerifiedBrowserBundleImage {
    pub fn release(&self) -> &VerifiedHostReleaseBytes {
        &self.release
    }
    pub fn distribution(&self) -> &VerifiedHostReleaseBytes {
        &self.distribution
    }
    pub fn generic_image_id(&self) -> &str {
        &self.generic_image_id
    }
    pub fn bundle_image_id(&self) -> &str {
        &self.bundle_image_id
    }
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
    pub fn configuration_id(&self) -> &str {
        &self.configuration_id
    }
    pub fn image_content_digest(&self) -> &str {
        self.release.image_content_digest()
    }
}

/// Both byteproofs must come from their actual opened, owner-authorized payloads.
/// This adds target semantics, not source/tool authority, initialization or Boot.
/// `host_image` remains the generic Make IMAGE used by existing Spore sealing;
/// the independently computed browser IMAGE is retained under its own identity.
pub fn verify_browser_bundle_image(
    checked: &CheckedHostConfiguration,
    host_image: &HostImage,
    release: VerifiedHostReleaseBytes,
    distribution: VerifiedHostReleaseBytes,
    browser_image_bytes: &[u8],
) -> Result<VerifiedBrowserBundleImage, String> {
    let r = release.manifest();
    let d = distribution.manifest();
    let profile_id = format!(
        "sha256:{:x}",
        Sha256::digest(
            canonical_profile_json(checked.profile())
                .map_err(|e| format!("browser PROFILE: {e:?}"))?
        )
    );
    let configuration_id = checked.configuration_id();
    if checked.profile().target.key() != TARGET
        || host_image.manifest.target != TARGET
        || host_image.manifest.output != SporeOutputKind::BrowserBundle
        || host_image.manifest.profile_id != profile_id
        || host_image.manifest.source_configuration_id.as_deref() != Some(configuration_id)
        || host_image.manifest.source_identity != r.source_identity
        || r.source_identity != d.source_identity
        || r.target_id != TARGET
        || d.target_id != TARGET
        || r.make_package_id != "browser-wasm@1"
        || d.make_package_id != "browser-wasm@1"
        || r.output != "browser-bundle"
        || d.output != "browser-bundle"
        || r.builder_adapter != "conduit-host-browser/bind-prebuilt@1"
        || d.builder_adapter != "conduit-host-browser/build-wasm@1"
        || r.deployment_adapter != "conduit-host-browser/load@1"
        || d.deployment_adapter != r.deployment_adapter
    {
        return Err("BrowserBundle differs from exact checked Host/Make/source identity".into());
    }
    let reviewed: Distribution = serde_json::from_value(
        d.target_metadata
            .get("reviewed_distribution")
            .cloned()
            .ok_or("browser reviewed distribution absent")?,
    )
    .map_err(|e| format!("browser reviewed distribution: {e}"))?;
    validate_distribution(&reviewed, d, distribution.payload_bytes())?;
    if release.payload_prefix("runtime.wasm") != Some(b"\0asm\x01\0\0\0".as_slice())
        || distribution.payload_prefix("runtime.wasm") != release.payload_prefix("runtime.wasm")
    {
        return Err("BrowserBundle runtime has no observed Wasm version header".into());
    }
    if r.files.len() != d.files.len() + 1
        || r.files.len() > 16
        || r.files.iter().filter(|f| f.path == IMAGE).count() != 1
    {
        return Err("BrowserBundle IMAGE asset closure differs from distribution".into());
    }
    for file in &d.files {
        if r.files.iter().find(|f| f.path == file.path) != Some(file) {
            return Err("BrowserBundle changed a reviewed distribution payload".into());
        }
    }
    let image_file = r.files.iter().find(|f| f.path == IMAGE).unwrap();
    if browser_image_bytes.is_empty()
        || browser_image_bytes.len() > 256 * 1024
        || image_file.bytes != browser_image_bytes.len() as u64
        || image_file.sha256 != sha(browser_image_bytes)
    {
        return Err("BrowserBundle rich IMAGE bytes do not match verified release".into());
    }
    let providers = reviewed
        .implementations
        .iter()
        .map(|i| (i.id.as_str(), i))
        .collect::<BTreeMap<_, _>>();
    let selected = checked
        .resolved_bases()
        .iter()
        .map(|(_, id)| id.as_str())
        .collect::<BTreeSet<_>>();
    let mut implementations = Vec::with_capacity(selected.len());
    for id in selected {
        let provider = providers
            .get(id)
            .ok_or("selected browser implementation absent from reviewed distribution")?;
        let canonical = BROWSER_IMPLEMENTATIONS
            .iter()
            .find(|p| p.implementation_id == id)
            .ok_or("selected browser implementation is not supported by this Make package")?;
        if provider.revision != canonical.implementation_revision
            || provider.artifact != canonical.artifact
        {
            return Err("selected browser implementation revision/artifact mismatch".into());
        }
        implementations.push((*provider).clone());
    }
    let build_id = format!(
        "build:{}",
        sha(&serde_json::to_vec(&BuildBasis {
            schema: "conduit.browser/bundle-build-basis@1",
            target_id: TARGET,
            profile_id: &profile_id,
            source_configuration_id: configuration_id,
            distribution_id: &reviewed.distribution_id,
            distribution_digest: &d.bundle_sha256,
            runtime_abi: &reviewed.runtime_abi,
            implementations: &implementations,
        })
        .map_err(|e| e.to_string())?)
    );
    let boot = d
        .files
        .iter()
        .find(|f| f.path == BOOT)
        .ok_or("browser profile-gated Boot module absent")?;
    let payload = ImagePayload {
        schema: "conduit.browser/bundle-image@1",
        build_id: &build_id,
        target_id: TARGET,
        profile_id: &profile_id,
        source_configuration_id: configuration_id,
        reviewed_distribution: DistributionBinding {
            distribution_id: &reviewed.distribution_id,
            distribution_digest: &d.bundle_sha256,
            runtime_abi: &reviewed.runtime_abi,
            toolchain_identity: &reviewed.toolchain_identity,
            source_commit: &reviewed.source_commit,
        },
        implementations: &implementations,
        boot_module: BootModule {
            role: "profile-gated-boot",
            path: BOOT,
            sha256: &boot.sha256,
        },
        files: &d.files,
    };
    let bundle_image_id = format!(
        "image:{}",
        sha(&serde_json::to_vec(&payload).map_err(|e| e.to_string())?)
    );
    let mut expected = serde_json::to_value(&payload).map_err(|e| e.to_string())?;
    expected
        .as_object_mut()
        .unwrap()
        .insert("image_id".into(), bundle_image_id.clone().into());
    let actual: serde_json::Value = serde_json::from_slice(browser_image_bytes)
        .map_err(|e| format!("browser rich IMAGE: {e}"))?;
    if actual != expected {
        return Err(
            "BrowserBundle rich IMAGE differs from checked profile/distribution/build basis".into(),
        );
    }
    // Boot recomputes JSON.stringify(parsed payload), whose field order is
    // significant. Value equality alone would accept reordered keys that Boot
    // rejects. Accept the existing producer's exact spelling/order, with its
    // optional pretty-print whitespace; alternate JSON escape spellings refuse.
    let document = serde_json::to_vec(&ImageDocument {
        payload: &payload,
        image_id: &bundle_image_id,
    })
    .map_err(|e| e.to_string())?;
    if compact_producer_json(browser_image_bytes) != document {
        return Err(
            "BrowserBundle rich IMAGE does not retain canonical producer field order/spelling"
                .into(),
        );
    }
    for (key, value) in [
        ("browser_image_id", bundle_image_id.as_str()),
        ("browser_build_id", build_id.as_str()),
        ("browser_profile_id", profile_id.as_str()),
        ("browser_configuration_id", configuration_id),
        ("distribution_id", reviewed.distribution_id.as_str()),
        ("distribution_sha256", d.bundle_sha256.as_str()),
    ] {
        if r.target_metadata.get(key).and_then(|v| v.as_str()) != Some(value) {
            return Err(format!("BrowserBundle release lost exact {key}"));
        }
    }
    Ok(VerifiedBrowserBundleImage {
        generic_image_id: host_image.manifest.image_id.clone(),
        bundle_image_id,
        profile_id,
        configuration_id: configuration_id.into(),
        release,
        distribution,
    })
}

fn sha(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn compact_producer_json(bytes: &[u8]) -> Vec<u8> {
    let mut compact = Vec::with_capacity(bytes.len());
    let mut in_string = false;
    let mut escaped = false;
    for &byte in bytes {
        if in_string {
            compact.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'"' {
            in_string = true;
            compact.push(byte);
        } else if !byte.is_ascii_whitespace() {
            compact.push(byte);
        }
    }
    compact
}
fn bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256
}
fn validate_distribution(
    d: &Distribution,
    m: &HostReleaseManifest,
    total: u64,
) -> Result<(), String> {
    if d.schema != DISTRIBUTION
        || d.distribution_id != DISTRIBUTION
        || d.runtime_abi != "conduit.browser/runtime-abi@1"
        || !d.targets.iter().any(|t| t == TARGET)
        || !bounded(&d.toolchain_identity)
        || d.source_commit != m.source_identity
        || !bounded(&d.source_commit)
        || d.maximum_bundle_bytes == 0
        || total > d.maximum_bundle_bytes
        || d.implementations.is_empty()
        || d.implementations.len() > 64
        || d.modules.len() > 32
        || m.files.is_empty()
        || m.files.len() >= 16
    {
        return Err("browser reviewed distribution declaration incompatible".into());
    }
    let mut ids = BTreeSet::new();
    for i in &d.implementations {
        let path = if i.artifact == crate::REVIEWED_RUNTIME_ARTIFACT {
            "runtime.wasm"
        } else {
            i.artifact.as_str()
        };
        if !bounded(&i.id)
            || i.revision < 1
            || !bounded(&i.artifact)
            || !ids.insert(&i.id)
            || !m.files.iter().any(|f| f.path == path)
        {
            return Err("browser distribution implementation table invalid".into());
        }
    }
    let modules = d
        .modules
        .iter()
        .map(|module| (module.path.as_str(), module))
        .collect::<BTreeMap<_, _>>();
    if modules.len() != d.modules.len() {
        return Err("browser module declarations duplicated".into());
    }
    for module in &d.modules {
        if !bounded(&module.path)
            || module.dependencies.len() > 16
            || !m.files.iter().any(|f| f.path == module.path)
        {
            return Err("browser module declaration is missing or unbounded".into());
        }
        visit(
            &module.path,
            &modules,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
        )?;
    }
    for f in &m.files {
        let maximum = if f.path == "runtime.wasm" {
            20 * 1024 * 1024
        } else {
            256 * 1024
        };
        if f.bytes > maximum {
            return Err("browser payload exceeds target consumer bound".into());
        }
    }
    Ok(())
}
fn visit<'a>(
    name: &'a str,
    modules: &BTreeMap<&'a str, &'a Module>,
    active: &mut BTreeSet<&'a str>,
    done: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    if done.contains(name) {
        return Ok(());
    }
    if !active.insert(name) {
        return Err("browser module dependency cycle".into());
    }
    let module = modules
        .get(name)
        .ok_or("browser module dependency absent")?;
    for dependency in &module.dependencies {
        visit(dependency, modules, active, done)?;
    }
    active.remove(name);
    done.insert(name);
    Ok(())
}

#[cfg(test)]
mod tests;
