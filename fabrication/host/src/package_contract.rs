use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{HostBounds, HostProfile, SporeOutputKind};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrerequisiteNode {
    Implementation(String),
    HostOperation(String),
    Resource(String),
    Base(String),
    Driver(String),
    Facility(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementationMetadata {
    pub kind: String,
    pub contract_revision: String,
    pub targets: Vec<String>,
    pub prerequisites: Vec<PrerequisiteNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenterMetadata {
    pub targets: Vec<String>,
    pub prerequisites: Vec<PrerequisiteNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageCatalogContribution {
    pub implementations: BTreeMap<String, ImplementationMetadata>,
    pub presenters: BTreeMap<String, PresenterMetadata>,
    pub dependencies: BTreeMap<PrerequisiteNode, Vec<PrerequisiteNode>>,
    pub facilities: Vec<String>,
    pub profile_fragments: Vec<String>,
    pub mutually_exclusive_mechanisms: Vec<(String, String)>,
}

pub const FABRICATION_PACKAGE_CONTRACT: &str = "conduit.host/fabrication-package@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PostBuildAction {
    Launch,
    Load,
    Flash,
    Boot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationPackageProvenance {
    pub implementation_id: String,
    pub package_id: String,
    pub package_revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FabricationBuildSelection {
    pub fabrication_package_id: String,
    pub fabrication_package_revision: u32,
    pub toolchain_identity: String,
    pub builder_adapter: String,
    pub deployment_adapter: Option<String>,
    pub post_build_actions: Vec<PostBuildAction>,
    pub output: SporeOutputKind,
    pub maxima: HostBounds,
    pub features: Vec<String>,
    pub selected_base_implementations: Vec<String>,
    pub implementation_packages: Vec<ImplementationPackageProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImplementationOffer {
    pub base_kind: String,
    pub implementation_id: String,
    pub implementation_revision: u32,
    pub target_patterns: Vec<String>,
    pub prerequisites: Vec<String>,
    pub build_feature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDescriptor {
    pub label: String,
    pub family: String,
    pub architecture: String,
    pub machine: String,
    pub board: Option<String>,
    pub os: Option<String>,
    pub host_core: String,
    pub presenter: Option<TargetPresenter>,
    pub host_operations: Vec<String>,
    pub toolchain_identity: String,
    pub builder_adapter: String,
    pub deployment_adapter: Option<String>,
    pub outputs: Vec<SporeOutputKind>,
    pub default_output: SporeOutputKind,
    pub post_build_actions: Vec<PostBuildAction>,
    /// Exact package-owned descriptor bindings accepted for this target.
    pub fabrication_descriptors: Vec<String>,
    pub maxima: HostBounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPresenter {
    pub id: String,
    pub implementation_id: String,
    pub interactive: bool,
}

impl TargetDescriptor {
    pub fn key(&self) -> String {
        format!("{}/{}/{}", self.family, self.architecture, self.machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabricationAnchor {
    pub package_id: String,
    pub package_revision: u32,
    pub targets: Vec<TargetDescriptor>,
    pub offers: Vec<ImplementationOffer>,
    pub catalog: PackageCatalogContribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabricationExtension {
    pub package_id: String,
    pub package_revision: u32,
    pub compatible_target_patterns: Vec<String>,
    pub offers: Vec<ImplementationOffer>,
    pub catalog: PackageCatalogContribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FabricationContribution {
    Anchor(FabricationAnchor),
    Extension(FabricationExtension),
}

impl FabricationContribution {
    pub fn package_id(&self) -> &str {
        match self {
            Self::Anchor(anchor) => &anchor.package_id,
            Self::Extension(extension) => &extension.package_id,
        }
    }

    pub fn package_revision(&self) -> u32 {
        match self {
            Self::Anchor(anchor) => anchor.package_revision,
            Self::Extension(extension) => extension.package_revision,
        }
    }

    pub fn offers(&self) -> &[ImplementationOffer] {
        match self {
            Self::Anchor(anchor) => &anchor.offers,
            Self::Extension(extension) => &extension.offers,
        }
    }

    pub fn catalog(&self) -> &PackageCatalogContribution {
        match self {
            Self::Anchor(anchor) => &anchor.catalog,
            Self::Extension(extension) => &extension.catalog,
        }
    }
}

/// Lightweight descriptor entrypoint implemented by independently authored packages.
/// Heavy toolchains and builders remain behind the adapter identity until BUILD.
pub trait HostFabricationPackage {
    fn contribution(&self) -> FabricationContribution;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImplementationOffer {
    pub package_id: String,
    pub package_revision: u32,
    pub offer: ImplementationOffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCompositionDiagnostic {
    DuplicatePackageIdentity {
        package_id: String,
    },
    DuplicateTargetAnchor {
        target: String,
        packages: Vec<String>,
    },
    DuplicateImplementationIdentity {
        implementation_id: String,
        packages: Vec<String>,
    },
    DuplicateFabricationDescriptorBinding {
        binding: String,
        targets: Vec<String>,
    },
    ExtensionHasNoCompatibleAnchor {
        package_id: String,
    },
    OfferOutsidePackageTargets {
        package_id: String,
        implementation_id: String,
    },
    UnknownTarget {
        target: String,
    },
    UnsupportedOutput {
        package_id: String,
        output: SporeOutputKind,
    },
    UnsupportedImplementation {
        target: String,
        base_kind: String,
        implementation_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabricationPackageSet {
    contributions: Vec<FabricationContribution>,
}

impl FabricationPackageSet {
    pub fn compose(
        packages: &[&dyn HostFabricationPackage],
    ) -> Result<Self, Vec<PackageCompositionDiagnostic>> {
        Self::from_contributions(packages.iter().map(|package| package.contribution()))
    }

    pub fn from_contributions(
        contributions: impl IntoIterator<Item = FabricationContribution>,
    ) -> Result<Self, Vec<PackageCompositionDiagnostic>> {
        let mut contributions = contributions.into_iter().collect::<Vec<_>>();
        contributions.sort_by(|left, right| {
            left.package_id()
                .cmp(right.package_id())
                .then(left.package_revision().cmp(&right.package_revision()))
        });
        let mut diagnostics = Vec::new();

        for pair in contributions.windows(2) {
            if pair[0].package_id() == pair[1].package_id() {
                diagnostics.push(PackageCompositionDiagnostic::DuplicatePackageIdentity {
                    package_id: pair[0].package_id().into(),
                });
            }
        }

        let mut target_anchors = BTreeMap::<String, Vec<String>>::new();
        for contribution in &contributions {
            if let FabricationContribution::Anchor(anchor) = contribution {
                for target in &anchor.targets {
                    target_anchors
                        .entry(target.key())
                        .or_default()
                        .push(anchor.package_id.clone());
                }
            }
        }
        for (target, packages) in &target_anchors {
            if packages.len() > 1 {
                diagnostics.push(PackageCompositionDiagnostic::DuplicateTargetAnchor {
                    target: target.clone(),
                    packages: packages.clone(),
                });
            }
        }

        let mut descriptor_targets = BTreeMap::<String, Vec<String>>::new();
        for contribution in &contributions {
            if let FabricationContribution::Anchor(anchor) = contribution {
                for target in &anchor.targets {
                    for binding in &target.fabrication_descriptors {
                        descriptor_targets
                            .entry(binding.clone())
                            .or_default()
                            .push(target.key());
                    }
                }
            }
        }
        for (binding, mut targets) in descriptor_targets {
            targets.sort();
            targets.dedup();
            if targets.len() > 1 {
                diagnostics.push(
                    PackageCompositionDiagnostic::DuplicateFabricationDescriptorBinding {
                        binding,
                        targets,
                    },
                );
            }
        }

        let target_keys = target_anchors
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let mut implementations = BTreeMap::<String, Vec<String>>::new();
        for contribution in &contributions {
            let package_patterns = match contribution {
                FabricationContribution::Anchor(anchor) => anchor
                    .targets
                    .iter()
                    .map(TargetDescriptor::key)
                    .collect::<Vec<_>>(),
                FabricationContribution::Extension(extension) => extension
                    .compatible_target_patterns
                    .iter()
                    .map(|pattern| (*pattern).to_owned())
                    .collect(),
            };
            if matches!(contribution, FabricationContribution::Extension(_))
                && !package_patterns.iter().any(|pattern| {
                    target_keys
                        .iter()
                        .any(|target| target_matches(pattern, target))
                })
            {
                diagnostics.push(
                    PackageCompositionDiagnostic::ExtensionHasNoCompatibleAnchor {
                        package_id: contribution.package_id().into(),
                    },
                );
            }
            for offer in contribution.offers() {
                if !offer.target_patterns.iter().any(|offer_pattern| {
                    package_patterns
                        .iter()
                        .any(|package_pattern| patterns_overlap(offer_pattern, package_pattern))
                }) {
                    diagnostics.push(PackageCompositionDiagnostic::OfferOutsidePackageTargets {
                        package_id: contribution.package_id().into(),
                        implementation_id: offer.implementation_id.clone(),
                    });
                }
                implementations
                    .entry(offer.implementation_id.clone())
                    .or_default()
                    .push(contribution.package_id().into());
            }
        }
        for (implementation_id, mut packages) in implementations {
            packages.sort();
            packages.dedup();
            if packages.len() > 1 {
                diagnostics.push(
                    PackageCompositionDiagnostic::DuplicateImplementationIdentity {
                        implementation_id,
                        packages,
                    },
                );
            }
        }

        if diagnostics.is_empty() {
            Ok(Self { contributions })
        } else {
            Err(diagnostics)
        }
    }

    pub fn contributions(&self) -> &[FabricationContribution] {
        &self.contributions
    }

    pub fn target_descriptors(&self) -> Vec<&TargetDescriptor> {
        let mut targets = self
            .contributions
            .iter()
            .filter_map(|contribution| match contribution {
                FabricationContribution::Anchor(anchor) => Some(anchor.targets.as_slice()),
                FabricationContribution::Extension(_) => None,
            })
            .flatten()
            .collect::<Vec<_>>();
        targets.sort_by_key(|target| target.key());
        targets
    }

    pub fn anchor_for_target(&self, target: &str) -> Option<&FabricationAnchor> {
        self.contributions
            .iter()
            .find_map(|contribution| match contribution {
                FabricationContribution::Anchor(anchor)
                    if anchor.targets.iter().any(|item| item.key() == target) =>
                {
                    Some(anchor)
                }
                _ => None,
            })
    }

    pub fn target_descriptor(&self, target: &str) -> Option<&TargetDescriptor> {
        self.anchor_for_target(target)
            .and_then(|anchor| anchor.targets.iter().find(|item| item.key() == target))
    }

    pub fn offers_for_target(&self, target: &str) -> Vec<ResolvedImplementationOffer> {
        let mut offers = self
            .contributions
            .iter()
            .flat_map(|contribution| {
                contribution
                    .offers()
                    .iter()
                    .filter(move |offer| {
                        offer
                            .target_patterns
                            .iter()
                            .any(|pattern| target_matches(pattern, target))
                    })
                    .map(|offer| ResolvedImplementationOffer {
                        package_id: contribution.package_id().into(),
                        package_revision: contribution.package_revision(),
                        offer: offer.clone(),
                    })
            })
            .collect::<Vec<_>>();
        offers.sort_by(|left, right| {
            left.offer
                .implementation_id
                .cmp(&right.offer.implementation_id)
                .then(left.package_id.cmp(&right.package_id))
        });
        offers
    }

    pub fn derive_build_selection(
        &self,
        profile: &HostProfile,
        output: &SporeOutputKind,
    ) -> Result<FabricationBuildSelection, PackageCompositionDiagnostic> {
        let target = profile.target.key();
        let anchor = self.anchor_for_target(&target).ok_or_else(|| {
            PackageCompositionDiagnostic::UnknownTarget {
                target: target.clone(),
            }
        })?;
        let target_descriptor = anchor
            .targets
            .iter()
            .find(|candidate| candidate.key() == target)
            .expect("resolved anchor owns exact target");
        if !target_descriptor.outputs.contains(output) {
            return Err(PackageCompositionDiagnostic::UnsupportedOutput {
                package_id: anchor.package_id.clone(),
                output: output.clone(),
            });
        }
        let offers = self.offers_for_target(&target);
        let mut features = Vec::new();
        let mut implementations = Vec::new();
        let mut implementation_packages = Vec::new();
        for base in &profile.bases {
            let resolved = offers.iter().find(|candidate| {
                candidate.offer.base_kind == base.kind
                    && candidate.offer.implementation_id == base.driver
            });
            let Some(resolved) = resolved else {
                return Err(PackageCompositionDiagnostic::UnsupportedImplementation {
                    target,
                    base_kind: base.kind.clone(),
                    implementation_id: base.driver.clone(),
                });
            };
            implementations.push(base.driver.clone());
            if let Some(feature) = &resolved.offer.build_feature {
                features.push(feature.clone());
            }
            implementation_packages.push(ImplementationPackageProvenance {
                implementation_id: base.driver.clone(),
                package_id: resolved.package_id.clone(),
                package_revision: resolved.package_revision,
            });
        }
        features.sort();
        features.dedup();
        implementations.sort();
        implementation_packages.sort_by(|left, right| {
            left.implementation_id
                .cmp(&right.implementation_id)
                .then(left.package_id.cmp(&right.package_id))
        });
        Ok(FabricationBuildSelection {
            fabrication_package_id: anchor.package_id.clone(),
            fabrication_package_revision: anchor.package_revision,
            toolchain_identity: target_descriptor.toolchain_identity.clone(),
            builder_adapter: target_descriptor.builder_adapter.clone(),
            deployment_adapter: target_descriptor.deployment_adapter.clone(),
            post_build_actions: target_descriptor.post_build_actions.clone(),
            output: output.clone(),
            maxima: target_descriptor.maxima.clone(),
            features,
            selected_base_implementations: implementations,
            implementation_packages,
        })
    }
}

fn target_matches(pattern: &str, target: &str) -> bool {
    let pattern = pattern.split('/').collect::<Vec<_>>();
    let target = target.split('/').collect::<Vec<_>>();
    pattern.len() == target.len()
        && pattern
            .iter()
            .zip(target)
            .all(|(expected, found)| *expected == "*" || *expected == found)
}

fn patterns_overlap(left: &str, right: &str) -> bool {
    let left = left.split('/').collect::<Vec<_>>();
    let right = right.split('/').collect::<Vec<_>>();
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| *left == right || *left == "*" || right == "*")
}
