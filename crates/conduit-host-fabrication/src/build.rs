use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    validate_profile, BaseSelection, DriverSelection, FabricationCatalog, FabricationPackageSet,
    HostBounds, HostProfile, ImplementationPackageProvenance, PackageCompositionDiagnostic,
    PostBuildAction, ProfileDiagnostic, ResourceBudget, SporeOutputKind,
};

pub const BUILD_MANIFEST_SCHEMA: &str = "conduit.host/build-manifest@2";
pub const IMAGE_SCHEMA: &str = "conduit.host/image@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDiagnostic {
    Profile(ProfileDiagnostic),
    Package(PackageCompositionDiagnostic),
    SourceIdentityMissing,
    ToolchainUnavailable {
        toolchain: String,
    },
    ResourceBudgetOverflow {
        field: &'static str,
        requested: u64,
        maximum: u64,
    },
    MutuallyExclusiveMechanisms {
        left: String,
        right: String,
    },
    ArtifactBindingMismatch {
        expected: String,
        found: String,
    },
    Encoding {
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInputs {
    pub source_identity: String,
    pub toolchain_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reproducibility {
    ByteExact,
    ManifestExactArtifactToolchainDependent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildManifest {
    pub schema: String,
    pub profile_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_configuration_id: Option<String>,
    pub build_id: String,
    pub image_id: String,
    pub source_identity: String,
    pub toolchain_identity: String,
    pub fabrication_package_id: String,
    pub fabrication_package_revision: u32,
    pub builder_adapter: String,
    pub deployment_adapter: Option<String>,
    pub output: SporeOutputKind,
    pub implementation_packages: Vec<ImplementationPackageProvenance>,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fabrication_descriptor: Option<String>,
    pub profile_fragments: Vec<String>,
    pub post_build_actions: Vec<PostBuildAction>,
    pub reproducibility: Reproducibility,
    pub implementations: Vec<String>,
    pub host_operations: Vec<String>,
    pub resources: Vec<String>,
    pub resource_budgets: Vec<ResourceBudget>,
    pub bases: Vec<String>,
    pub base_selections: Vec<BaseSelection>,
    pub drivers: Vec<String>,
    pub driver_selections: Vec<DriverSelection>,
    pub lines: Vec<String>,
    pub presenters: Vec<String>,
    pub facilities: Vec<String>,
    pub bounds: HostBounds,
    pub inclusion_paths: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImagePayload {
    pub schema: String,
    pub build_id: String,
    pub profile_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_configuration_id: Option<String>,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fabrication_descriptor: Option<String>,
    pub profile_fragments: Vec<String>,
    pub implementations: Vec<String>,
    pub host_operations: Vec<String>,
    pub resources: Vec<String>,
    pub resource_budgets: Vec<ResourceBudget>,
    pub bases: Vec<String>,
    pub base_selections: Vec<BaseSelection>,
    pub drivers: Vec<String>,
    pub driver_selections: Vec<DriverSelection>,
    pub lines: Vec<String>,
    pub presenters: Vec<String>,
    pub facilities: Vec<String>,
    pub bounds: HostBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostImage {
    pub payload: ImagePayload,
    pub manifest: BuildManifest,
}

pub fn build_default_host_image(
    profile: HostProfile,
    catalog: &FabricationCatalog,
    packages: &FabricationPackageSet,
    inputs: &BuildInputs,
) -> Result<(HostImage, Vec<u8>), Vec<BuildDiagnostic>> {
    let target = profile.target.key();
    let output = packages
        .target_descriptor(&target)
        .ok_or_else(|| {
            vec![BuildDiagnostic::Package(
                PackageCompositionDiagnostic::UnknownTarget { target },
            )]
        })?
        .default_output
        .clone();
    build_host_image(profile, catalog, packages, &output, inputs)
}

pub fn build_host_image(
    profile: HostProfile,
    catalog: &FabricationCatalog,
    packages: &FabricationPackageSet,
    output: &SporeOutputKind,
    inputs: &BuildInputs,
) -> Result<(HostImage, Vec<u8>), Vec<BuildDiagnostic>> {
    let validated = validate_profile(profile, catalog).map_err(|items| {
        items
            .into_iter()
            .map(BuildDiagnostic::Profile)
            .collect::<Vec<_>>()
    })?;
    let profile = validated.profile();
    let fabrication = packages
        .derive_build_selection(profile, output)
        .map_err(|diagnostic| vec![BuildDiagnostic::Package(diagnostic)])?;
    let mut diagnostics = Vec::new();
    if inputs.source_identity.trim().is_empty() {
        diagnostics.push(BuildDiagnostic::SourceIdentityMissing);
    }
    if !inputs.toolchain_available {
        diagnostics.push(BuildDiagnostic::ToolchainUnavailable {
            toolchain: fabrication.toolchain_identity.clone(),
        });
    }
    validate_bounds(&profile.bounds, &fabrication.maxima, &mut diagnostics);
    for (left, right) in [
        ("compositor/native@1", "browser/dom"),
        ("display/linear-framebuffer@1", "browser/dom@1"),
    ] {
        let selected = profile
            .facilities
            .iter()
            .chain(profile.bases.iter().map(|item| &item.kind))
            .chain(profile.drivers.iter().map(|item| &item.kind))
            .cloned()
            .collect::<BTreeSet<_>>();
        if selected.contains(left) && selected.contains(right) {
            diagnostics.push(BuildDiagnostic::MutuallyExclusiveMechanisms {
                left: left.into(),
                right: right.into(),
            });
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let target = profile.target.key();
    let reproducibility = if matches!(output, SporeOutputKind::BrowserBundle) {
        Reproducibility::ManifestExactArtifactToolchainDependent
    } else {
        Reproducibility::ByteExact
    };
    let implementations = sorted(
        profile
            .capabilities
            .iter()
            .map(|item| item.implementation.clone())
            .collect(),
    );
    let presenters = sorted(
        profile
            .presenters
            .iter()
            .map(|item| item.implementation.clone())
            .collect(),
    );
    let build_basis = serde_json::to_vec(&(
        validated.profile_id().as_str(),
        &inputs.source_identity,
        &fabrication.fabrication_package_id,
        fabrication.fabrication_package_revision,
        &fabrication.toolchain_identity,
        &fabrication.builder_adapter,
        &fabrication.output,
        &fabrication.features,
        &fabrication.implementation_packages,
        &target,
        &implementations,
        &presenters,
        &profile.bounds,
    ))
    .map_err(|error| {
        vec![BuildDiagnostic::Encoding {
            detail: error.to_string(),
        }]
    })?;
    let build_id = digest("build", &build_basis);
    let payload = ImagePayload {
        schema: IMAGE_SCHEMA.into(),
        build_id: build_id.clone(),
        profile_id: validated.profile_id().as_str().into(),
        source_configuration_id: profile.source_configuration_id.clone(),
        target: target.clone(),
        fabrication_descriptor: profile.target.fabrication_descriptor.clone(),
        profile_fragments: sorted(profile.fragments.clone()),
        implementations: implementations.clone(),
        host_operations: sorted(profile.host_operations.clone()),
        resources: sorted(
            profile
                .resources
                .iter()
                .map(|item| item.id.clone())
                .collect(),
        ),
        resource_budgets: sorted_resources(profile.resources.clone()),
        bases: sorted(profile.bases.iter().map(|item| item.id.clone()).collect()),
        base_selections: sorted_bases(profile.bases.clone()),
        drivers: sorted(profile.drivers.iter().map(|item| item.id.clone()).collect()),
        driver_selections: sorted_drivers(profile.drivers.clone()),
        lines: sorted(profile.lines.clone()),
        presenters: presenters.clone(),
        facilities: sorted(profile.facilities.clone()),
        bounds: profile.bounds.clone(),
    };
    let payload_bytes = serde_json::to_vec(&payload).map_err(|error| {
        vec![BuildDiagnostic::Encoding {
            detail: error.to_string(),
        }]
    })?;
    let image_id = digest("image", &payload_bytes);
    let manifest = BuildManifest {
        schema: BUILD_MANIFEST_SCHEMA.into(),
        profile_id: validated.profile_id().as_str().into(),
        source_configuration_id: profile.source_configuration_id.clone(),
        build_id: build_id.clone(),
        image_id: image_id.clone(),
        source_identity: inputs.source_identity.clone(),
        toolchain_identity: fabrication.toolchain_identity.clone(),
        fabrication_package_id: fabrication.fabrication_package_id,
        fabrication_package_revision: fabrication.fabrication_package_revision,
        builder_adapter: fabrication.builder_adapter,
        deployment_adapter: fabrication.deployment_adapter,
        output: fabrication.output,
        implementation_packages: fabrication.implementation_packages,
        target,
        fabrication_descriptor: payload.fabrication_descriptor.clone(),
        profile_fragments: payload.profile_fragments.clone(),
        post_build_actions: fabrication.post_build_actions,
        reproducibility,
        implementations,
        host_operations: payload.host_operations.clone(),
        resources: payload.resources.clone(),
        resource_budgets: payload.resource_budgets.clone(),
        bases: payload.bases.clone(),
        base_selections: payload.base_selections.clone(),
        drivers: payload.drivers.clone(),
        driver_selections: payload.driver_selections.clone(),
        lines: payload.lines.clone(),
        presenters,
        facilities: payload.facilities.clone(),
        bounds: profile.bounds.clone(),
        inclusion_paths: validated.dependency_paths().clone(),
    };
    let image = HostImage { payload, manifest };
    let bytes = serde_json::to_vec_pretty(&image).map_err(|error| {
        vec![BuildDiagnostic::Encoding {
            detail: error.to_string(),
        }]
    })?;
    Ok((image, bytes))
}

pub fn verify_image_binding(image: &HostImage, bytes: &[u8]) -> Result<(), BuildDiagnostic> {
    let decoded: HostImage =
        serde_json::from_slice(bytes).map_err(|error| BuildDiagnostic::Encoding {
            detail: error.to_string(),
        })?;
    let found = serde_json::to_vec(&decoded.payload)
        .map(|payload| digest("image", &payload))
        .map_err(|error| BuildDiagnostic::Encoding {
            detail: error.to_string(),
        })?;
    if decoded.manifest.image_id != found || &decoded != image {
        return Err(BuildDiagnostic::ArtifactBindingMismatch {
            expected: image.manifest.image_id.clone(),
            found,
        });
    }
    Ok(())
}

fn validate_bounds(
    requested: &HostBounds,
    maxima: &HostBounds,
    diagnostics: &mut Vec<BuildDiagnostic>,
) {
    macro_rules! check {
        ($field:ident) => {
            if requested.$field > maxima.$field {
                diagnostics.push(BuildDiagnostic::ResourceBudgetOverflow {
                    field: stringify!($field),
                    requested: requested.$field as u64,
                    maximum: maxima.$field as u64,
                });
            }
        };
    }
    check!(static_memory_bytes);
    check!(heap_arena_bytes);
    check!(queue_items);
    check!(buffered_bytes);
    check!(active_instances);
    check!(operation_slots);
    check!(timer_slots);
    check!(line_sessions);
    check!(evidence_items);
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}
fn sorted_resources(mut values: Vec<ResourceBudget>) -> Vec<ResourceBudget> {
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}
fn sorted_bases(mut values: Vec<BaseSelection>) -> Vec<BaseSelection> {
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}
fn sorted_drivers(mut values: Vec<DriverSelection>) -> Vec<DriverSelection> {
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}
fn digest(prefix: &str, bytes: &[u8]) -> String {
    format!("{prefix}:sha256:{:x}", Sha256::digest(bytes))
}
