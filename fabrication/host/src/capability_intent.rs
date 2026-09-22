//! Purpose-first capability review over package-owned fabrication truth.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{FabricationCatalog, FabricationPackageSet, PrerequisiteNode};

pub const MAX_CAPABILITY_INTENTS: usize = 32;
pub const MAX_IMPLEMENTATION_CANDIDATES: usize = 16;
pub const MAX_PREREQUISITES_PER_CANDIDATE: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityIntent {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_implementation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostCapabilityIntent {
    pub target: String,
    pub capabilities: Vec<CapabilityIntent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseImplementationCandidate {
    pub implementation_id: String,
    pub implementation_revision: u32,
    pub package_id: String,
    pub package_revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredBaseReview {
    pub base_kind: String,
    pub compatible_implementations: Vec<BaseImplementationCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CapabilityPrerequisiteReview {
    pub implementations: Vec<String>,
    pub host_calls: Vec<String>,
    pub resources: Vec<String>,
    pub bases: Vec<RequiredBaseReview>,
    pub drivers: Vec<String>,
    pub facilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityImplementationCandidate {
    pub implementation_id: String,
    pub contract_revision: String,
    pub prerequisites: CapabilityPrerequisiteReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedCapabilityIntent {
    pub kind: String,
    pub pinned: bool,
    pub candidates: Vec<CapabilityImplementationCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCapabilityReview {
    pub target: String,
    pub capabilities: Vec<ResolvedCapabilityIntent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityIntentRefusal {
    UnknownTarget,
    EmptyCapabilities,
    TooManyCapabilities,
    EmptyCapabilityKind,
    DuplicateCapabilityKind {
        kind: String,
    },
    NoCompatibleImplementation {
        kind: String,
    },
    UnknownPinnedImplementation {
        implementation: String,
    },
    PinnedImplementationKindMismatch {
        implementation: String,
        kind: String,
    },
    PinnedImplementationTargetMismatch {
        implementation: String,
        target: String,
    },
    TooManyImplementationCandidates {
        kind: String,
    },
    TooManyPrerequisites {
        implementation: String,
    },
    CircularPrerequisite {
        implementation: String,
    },
    MissingBaseImplementation {
        base_kind: String,
        target: String,
    },
}

/// Resolve semantic capability intent to exact, inspectable candidates.
///
/// This does not choose a candidate, create a PROFILE, fabricate an artifact,
/// initialize a Base, or claim a runtime offer.
pub fn resolve_host_capability_intent(
    intent: &HostCapabilityIntent,
    catalog: &FabricationCatalog,
    packages: &FabricationPackageSet,
) -> Result<HostCapabilityReview, CapabilityIntentRefusal> {
    if packages.target_descriptor(&intent.target).is_none() {
        return Err(CapabilityIntentRefusal::UnknownTarget);
    }
    if intent.capabilities.is_empty() {
        return Err(CapabilityIntentRefusal::EmptyCapabilities);
    }
    if intent.capabilities.len() > MAX_CAPABILITY_INTENTS {
        return Err(CapabilityIntentRefusal::TooManyCapabilities);
    }

    let mut seen = BTreeSet::new();
    let mut resolved = Vec::with_capacity(intent.capabilities.len());
    for capability in &intent.capabilities {
        if capability.kind.trim().is_empty() {
            return Err(CapabilityIntentRefusal::EmptyCapabilityKind);
        }
        if !seen.insert(capability.kind.as_str()) {
            return Err(CapabilityIntentRefusal::DuplicateCapabilityKind {
                kind: capability.kind.clone(),
            });
        }

        let metadata = if let Some(pinned) = &capability.pinned_implementation {
            let metadata = catalog.implementations.get(pinned).ok_or_else(|| {
                CapabilityIntentRefusal::UnknownPinnedImplementation {
                    implementation: pinned.clone(),
                }
            })?;
            if metadata.kind != capability.kind {
                return Err(CapabilityIntentRefusal::PinnedImplementationKindMismatch {
                    implementation: pinned.clone(),
                    kind: capability.kind.clone(),
                });
            }
            if !matches_target(&metadata.targets, &intent.target) {
                return Err(
                    CapabilityIntentRefusal::PinnedImplementationTargetMismatch {
                        implementation: pinned.clone(),
                        target: intent.target.clone(),
                    },
                );
            }
            vec![(pinned, metadata)]
        } else {
            catalog
                .implementations
                .iter()
                .filter(|(_, metadata)| {
                    metadata.kind == capability.kind
                        && matches_target(&metadata.targets, &intent.target)
                })
                .collect::<Vec<_>>()
        };
        if metadata.is_empty() {
            return Err(CapabilityIntentRefusal::NoCompatibleImplementation {
                kind: capability.kind.clone(),
            });
        }
        if metadata.len() > MAX_IMPLEMENTATION_CANDIDATES {
            return Err(CapabilityIntentRefusal::TooManyImplementationCandidates {
                kind: capability.kind.clone(),
            });
        }

        let mut candidates = Vec::with_capacity(metadata.len());
        for (implementation_id, metadata) in metadata {
            let prerequisites = prerequisite_review(
                implementation_id,
                &metadata.prerequisites,
                &intent.target,
                catalog,
                packages,
            )?;
            candidates.push(CapabilityImplementationCandidate {
                implementation_id: implementation_id.clone(),
                contract_revision: metadata.contract_revision.clone(),
                prerequisites,
            });
        }
        resolved.push(ResolvedCapabilityIntent {
            kind: capability.kind.clone(),
            pinned: capability.pinned_implementation.is_some(),
            candidates,
        });
    }

    Ok(HostCapabilityReview {
        target: intent.target.clone(),
        capabilities: resolved,
    })
}

fn prerequisite_review(
    implementation_id: &str,
    roots: &[PrerequisiteNode],
    target: &str,
    catalog: &FabricationCatalog,
    packages: &FabricationPackageSet,
) -> Result<CapabilityPrerequisiteReview, CapabilityIntentRefusal> {
    let mut nodes = BTreeSet::new();
    let mut visiting = Vec::new();
    for root in roots {
        collect_prerequisites(implementation_id, root, catalog, &mut visiting, &mut nodes)?;
    }
    if nodes.len() > MAX_PREREQUISITES_PER_CANDIDATE {
        return Err(CapabilityIntentRefusal::TooManyPrerequisites {
            implementation: implementation_id.into(),
        });
    }

    let mut review = CapabilityPrerequisiteReview::default();
    let offers = packages.offers_for_target(target);
    let mut bases = BTreeMap::<String, Vec<BaseImplementationCandidate>>::new();
    for node in nodes {
        match node {
            PrerequisiteNode::Implementation(value) => review.implementations.push(value),
            PrerequisiteNode::HostCall(value) => review.host_calls.push(value),
            PrerequisiteNode::Resource(value) => review.resources.push(value),
            PrerequisiteNode::Base(value) => {
                let candidates = offers
                    .iter()
                    .filter(|offer| offer.offer.base_kind == value)
                    .map(|offer| BaseImplementationCandidate {
                        implementation_id: offer.offer.implementation_id.clone(),
                        implementation_revision: offer.offer.implementation_revision,
                        package_id: offer.package_id.clone(),
                        package_revision: offer.package_revision,
                    })
                    .collect::<Vec<_>>();
                if candidates.is_empty() {
                    return Err(CapabilityIntentRefusal::MissingBaseImplementation {
                        base_kind: value,
                        target: target.into(),
                    });
                }
                bases.insert(value, candidates);
            }
            PrerequisiteNode::Driver(value) => review.drivers.push(value),
            PrerequisiteNode::Facility(value) => review.facilities.push(value),
        }
    }
    review.bases = bases
        .into_iter()
        .map(
            |(base_kind, compatible_implementations)| RequiredBaseReview {
                base_kind,
                compatible_implementations,
            },
        )
        .collect();
    Ok(review)
}

fn collect_prerequisites(
    implementation_id: &str,
    node: &PrerequisiteNode,
    catalog: &FabricationCatalog,
    visiting: &mut Vec<PrerequisiteNode>,
    nodes: &mut BTreeSet<PrerequisiteNode>,
) -> Result<(), CapabilityIntentRefusal> {
    if visiting.contains(node) {
        return Err(CapabilityIntentRefusal::CircularPrerequisite {
            implementation: implementation_id.into(),
        });
    }
    if nodes.insert(node.clone()) {
        visiting.push(node.clone());
        if let Some(dependencies) = catalog.dependencies.get(node) {
            for dependency in dependencies {
                collect_prerequisites(implementation_id, dependency, catalog, visiting, nodes)?;
            }
        }
        visiting.pop();
    }
    Ok(())
}

fn matches_target(patterns: &[String], target: &str) -> bool {
    patterns.iter().any(|pattern| {
        let pattern = pattern.split('/').collect::<Vec<_>>();
        let target = target.split('/').collect::<Vec<_>>();
        pattern.len() == target.len()
            && pattern
                .iter()
                .zip(target)
                .all(|(expected, found)| *expected == "*" || *expected == found)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        test_packages::{test_catalog, test_package_set},
        ImplementationMetadata, PackageCatalogContribution,
    };

    fn catalog() -> FabricationCatalog {
        let implementations = BTreeMap::from([
            (
                "capability/time-reader-a@1".into(),
                ImplementationMetadata {
                    kind: "time/read".into(),
                    contract_revision: "conduit.time/read@1".into(),
                    targets: vec!["std/*/*".into()],
                    prerequisites: vec![
                        PrerequisiteNode::Base("clock/monotonic".into()),
                        PrerequisiteNode::HostCall("conduit.host/clock-read@1".into()),
                    ],
                },
            ),
            (
                "capability/time-reader-b@1".into(),
                ImplementationMetadata {
                    kind: "time/read".into(),
                    contract_revision: "conduit.time/read@1".into(),
                    targets: vec!["std/x86_64/computer".into()],
                    prerequisites: vec![PrerequisiteNode::Base("clock/monotonic".into())],
                },
            ),
        ]);
        test_catalog().with_catalog_contribution(&PackageCatalogContribution {
            implementations,
            ..Default::default()
        })
    }

    fn intent(pin: Option<&str>) -> HostCapabilityIntent {
        HostCapabilityIntent {
            target: "std/x86_64/computer".into(),
            capabilities: vec![CapabilityIntent {
                kind: "time/read".into(),
                pinned_implementation: pin.map(Into::into),
            }],
        }
    }

    #[test]
    fn purpose_first_review_keeps_semantics_implementations_and_bases_distinct() {
        let review =
            resolve_host_capability_intent(&intent(None), &catalog(), &test_package_set()).unwrap();

        let capability = &review.capabilities[0];
        assert_eq!(capability.kind, "time/read");
        assert!(!capability.pinned);
        assert_eq!(capability.candidates.len(), 2);
        assert_eq!(
            capability.candidates[0].implementation_id,
            "capability/time-reader-a@1"
        );
        let base = &capability.candidates[0].prerequisites.bases[0];
        assert_eq!(base.base_kind, "clock/monotonic");
        assert_eq!(
            base.compatible_implementations[0].implementation_id,
            "hosted/monotonic-clock@1"
        );
    }

    #[test]
    fn advanced_pin_returns_only_the_exact_reviewed_implementation() {
        let review = resolve_host_capability_intent(
            &intent(Some("capability/time-reader-b@1")),
            &catalog(),
            &test_package_set(),
        )
        .unwrap();
        assert!(review.capabilities[0].pinned);
        assert_eq!(review.capabilities[0].candidates.len(), 1);
        assert_eq!(
            review.capabilities[0].candidates[0].implementation_id,
            "capability/time-reader-b@1"
        );
    }

    #[test]
    fn incompatible_and_unknown_pins_refuse_distinctly() {
        let catalog = catalog();
        let packages = test_package_set();
        let unknown = resolve_host_capability_intent(
            &intent(Some("capability/missing@1")),
            &catalog,
            &packages,
        );
        assert_eq!(
            unknown,
            Err(CapabilityIntentRefusal::UnknownPinnedImplementation {
                implementation: "capability/missing@1".into()
            })
        );

        let mut wrong_target = intent(Some("capability/time-reader-b@1"));
        wrong_target.target = "browser/wasm32/page".into();
        assert_eq!(
            resolve_host_capability_intent(&wrong_target, &catalog, &packages),
            Err(
                CapabilityIntentRefusal::PinnedImplementationTargetMismatch {
                    implementation: "capability/time-reader-b@1".into(),
                    target: "browser/wasm32/page".into()
                }
            )
        );
    }

    #[test]
    fn empty_duplicate_and_oversized_intent_refuse_before_resolution() {
        let catalog = catalog();
        let packages = test_package_set();
        let mut request = intent(None);
        request.capabilities.clear();
        assert_eq!(
            resolve_host_capability_intent(&request, &catalog, &packages),
            Err(CapabilityIntentRefusal::EmptyCapabilities)
        );

        request = intent(None);
        request.capabilities.push(request.capabilities[0].clone());
        assert_eq!(
            resolve_host_capability_intent(&request, &catalog, &packages),
            Err(CapabilityIntentRefusal::DuplicateCapabilityKind {
                kind: "time/read".into()
            })
        );

        request.capabilities = (0..=MAX_CAPABILITY_INTENTS)
            .map(|index| CapabilityIntent {
                kind: format!("capability/{index}"),
                pinned_implementation: None,
            })
            .collect();
        assert_eq!(
            resolve_host_capability_intent(&request, &catalog, &packages),
            Err(CapabilityIntentRefusal::TooManyCapabilities)
        );
    }
}
