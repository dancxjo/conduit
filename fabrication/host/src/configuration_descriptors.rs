use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    FabricationPackageSet, FabricationStrategy, HostBounds, PostBuildAction, SporeOutputKind,
    TargetDescriptor,
};

pub const FABRICATION_CHOOSER_CATALOG_SCHEMA: &str = "conduit.host/fabrication-chooser-catalog@1";

/// Portable, finite selection data derived from composed fabrication packages.
///
/// Product surfaces may render this projection, but do not own or extend it.
/// It describes construction choices only and carries no runtime observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FabricationChooserCatalog {
    pub schema: String,
    pub catalog_id: String,
    pub targets: Vec<FabricationTargetChoice>,
    pub does_not_create: Vec<FabricationSelectionNonEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FabricationTargetChoice {
    pub target_id: String,
    pub label: String,
    pub family: String,
    pub architecture: String,
    pub machine: String,
    pub board: Option<String>,
    pub os: Option<String>,
    pub fabrication_descriptors: Vec<String>,
    pub strategy: FabricationStrategy,
    pub toolchain_identity: String,
    pub builder_adapter: String,
    pub deployment_adapter: Option<String>,
    pub outputs: Vec<SporeOutputKind>,
    pub default_output: SporeOutputKind,
    pub post_build_actions: Vec<PostBuildAction>,
    pub maxima: HostBounds,
    pub bases: Vec<FabricationBaseChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FabricationBaseChoice {
    pub kind: String,
    pub implementations: Vec<FabricationImplementationChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FabricationImplementationChoice {
    pub implementation_id: String,
    pub implementation_revision: u32,
    pub package_id: String,
    pub package_revision: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FabricationSelectionNonEffect {
    HostIdentity,
    BootIdentity,
    BaseReadiness,
    CapabilityOffer,
    BodyMembership,
    Authority,
    Plan,
    Play,
}

pub fn fabrication_chooser_catalog(packages: &FabricationPackageSet) -> FabricationChooserCatalog {
    let targets = packages
        .target_descriptors()
        .into_iter()
        .map(|descriptor| target_choice(descriptor, packages))
        .collect();
    let does_not_create = vec![
        FabricationSelectionNonEffect::HostIdentity,
        FabricationSelectionNonEffect::BootIdentity,
        FabricationSelectionNonEffect::BaseReadiness,
        FabricationSelectionNonEffect::CapabilityOffer,
        FabricationSelectionNonEffect::BodyMembership,
        FabricationSelectionNonEffect::Authority,
        FabricationSelectionNonEffect::Plan,
        FabricationSelectionNonEffect::Play,
    ];
    let identity_basis = serde_json::to_vec(&(
        FABRICATION_CHOOSER_CATALOG_SCHEMA,
        &targets,
        &does_not_create,
    ))
    .expect("finite fabrication chooser catalog must encode");
    FabricationChooserCatalog {
        schema: FABRICATION_CHOOSER_CATALOG_SCHEMA.into(),
        catalog_id: format!("sha256:{:x}", Sha256::digest(identity_basis)),
        targets,
        does_not_create,
    }
}

fn target_choice(
    descriptor: &TargetDescriptor,
    packages: &FabricationPackageSet,
) -> FabricationTargetChoice {
    let mut bases = BTreeMap::<String, Vec<FabricationImplementationChoice>>::new();
    for resolved in packages.offers_for_target(&descriptor.key()) {
        bases
            .entry(resolved.offer.base_kind)
            .or_default()
            .push(FabricationImplementationChoice {
                implementation_id: resolved.offer.implementation_id,
                implementation_revision: resolved.offer.implementation_revision,
                package_id: resolved.package_id,
                package_revision: resolved.package_revision,
            });
    }
    let bases = bases
        .into_iter()
        .map(|(kind, mut implementations)| {
            implementations.sort_by(|left, right| {
                left.implementation_id
                    .cmp(&right.implementation_id)
                    .then(left.package_id.cmp(&right.package_id))
            });
            FabricationBaseChoice {
                kind,
                implementations,
            }
        })
        .collect();
    FabricationTargetChoice {
        target_id: descriptor.key(),
        label: descriptor.label.clone(),
        family: descriptor.family.clone(),
        architecture: descriptor.architecture.clone(),
        machine: descriptor.machine.clone(),
        board: descriptor.board.clone(),
        os: descriptor.os.clone(),
        fabrication_descriptors: descriptor.fabrication_descriptors.clone(),
        strategy: descriptor.strategy,
        toolchain_identity: descriptor.toolchain_identity.clone(),
        builder_adapter: descriptor.builder_adapter.clone(),
        deployment_adapter: descriptor.deployment_adapter.clone(),
        outputs: descriptor.outputs.clone(),
        default_output: descriptor.default_output.clone(),
        post_build_actions: descriptor.post_build_actions.clone(),
        maxima: descriptor.maxima.clone(),
        bases,
    }
}

pub fn compatible_base_implementations(
    descriptor: &TargetDescriptor,
    packages: &FabricationPackageSet,
) -> Vec<(String, Vec<String>)> {
    let mut choices = BTreeMap::<String, Vec<String>>::new();
    for resolved in packages.offers_for_target(&descriptor.key()) {
        choices
            .entry(resolved.offer.base_kind)
            .or_default()
            .push(resolved.offer.implementation_id);
    }
    choices
        .into_iter()
        .map(|(kind, mut implementations)| {
            implementations.sort();
            implementations.dedup();
            (kind, implementations)
        })
        .collect()
}
