use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    HostBounds, MakePackageSet, MakeStrategy, PostBuildAction, SporeOutputKind, TargetDescriptor,
};

pub const MAKE_CHOOSER_CATALOG_SCHEMA: &str = "conduit.host/make-chooser-catalog@1";

/// Portable, finite selection data derived from composed make packages.
///
/// Product surfaces may render this projection, but do not own or extend it.
/// It describes construction choices only and carries no runtime observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MakeChooserCatalog {
    pub schema: String,
    pub catalog_id: String,
    pub targets: Vec<MakeTargetChoice>,
    pub does_not_create: Vec<MakeSelectionNonEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MakeTargetChoice {
    pub target_id: String,
    pub label: String,
    pub family: String,
    pub architecture: String,
    pub machine: String,
    pub board: Option<String>,
    pub os: Option<String>,
    pub make_descriptors: Vec<String>,
    pub strategy: MakeStrategy,
    pub toolchain_identity: String,
    pub builder_adapter: String,
    pub deployment_adapter: Option<String>,
    pub outputs: Vec<SporeOutputKind>,
    pub default_output: SporeOutputKind,
    pub post_build_actions: Vec<PostBuildAction>,
    pub maxima: HostBounds,
    pub bases: Vec<MakeBaseChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MakeBaseChoice {
    pub kind: String,
    pub implementations: Vec<MakeImplementationChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MakeImplementationChoice {
    pub implementation_id: String,
    pub implementation_revision: u32,
    pub package_id: String,
    pub package_revision: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MakeSelectionNonEffect {
    HostIdentity,
    BootIdentity,
    BaseReadiness,
    CapabilityOffer,
    BodyMembership,
    Authority,
    Plan,
    Play,
}

pub fn make_chooser_catalog(packages: &MakePackageSet) -> MakeChooserCatalog {
    let targets = packages
        .target_descriptors()
        .into_iter()
        .map(|descriptor| target_choice(descriptor, packages))
        .collect();
    let does_not_create = vec![
        MakeSelectionNonEffect::HostIdentity,
        MakeSelectionNonEffect::BootIdentity,
        MakeSelectionNonEffect::BaseReadiness,
        MakeSelectionNonEffect::CapabilityOffer,
        MakeSelectionNonEffect::BodyMembership,
        MakeSelectionNonEffect::Authority,
        MakeSelectionNonEffect::Plan,
        MakeSelectionNonEffect::Play,
    ];
    let identity_basis =
        serde_json::to_vec(&(MAKE_CHOOSER_CATALOG_SCHEMA, &targets, &does_not_create))
            .expect("finite make chooser catalog must encode");
    MakeChooserCatalog {
        schema: MAKE_CHOOSER_CATALOG_SCHEMA.into(),
        catalog_id: format!("sha256:{:x}", Sha256::digest(identity_basis)),
        targets,
        does_not_create,
    }
}

fn target_choice(descriptor: &TargetDescriptor, packages: &MakePackageSet) -> MakeTargetChoice {
    let mut bases = BTreeMap::<String, Vec<MakeImplementationChoice>>::new();
    for resolved in packages.offers_for_target(&descriptor.key()) {
        bases
            .entry(resolved.offer.base_kind)
            .or_default()
            .push(MakeImplementationChoice {
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
            MakeBaseChoice {
                kind,
                implementations,
            }
        })
        .collect();
    MakeTargetChoice {
        target_id: descriptor.key(),
        label: descriptor.label.clone(),
        family: descriptor.family.clone(),
        architecture: descriptor.architecture.clone(),
        machine: descriptor.machine.clone(),
        board: descriptor.board.clone(),
        os: descriptor.os.clone(),
        make_descriptors: descriptor.make_descriptors.clone(),
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
    packages: &MakePackageSet,
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
