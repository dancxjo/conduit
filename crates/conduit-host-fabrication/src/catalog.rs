use std::collections::BTreeMap;

use conduit_std_catalog::supported_nucleus_offers;

use crate::{
    FabricationContribution, FabricationPackageSet, ImplementationMetadata,
    PackageCatalogContribution, PrerequisiteNode, PresenterMetadata,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabricationCatalog {
    pub implementations: BTreeMap<String, ImplementationMetadata>,
    pub presenters: BTreeMap<String, PresenterMetadata>,
    pub dependencies: BTreeMap<PrerequisiteNode, Vec<PrerequisiteNode>>,
    pub targets: Vec<String>,
    pub host_cores: Vec<String>,
    pub base_kinds: Vec<String>,
    pub base_targets: BTreeMap<String, Vec<String>>,
    pub driver_kinds: Vec<String>,
    pub driver_targets: BTreeMap<String, Vec<String>>,
    pub line_facilities: Vec<String>,
    pub facilities: Vec<String>,
    pub policy_profiles: Vec<String>,
    pub profile_fragments: Vec<String>,
    pub mutually_exclusive_mechanisms: Vec<(String, String)>,
    pub fabrication_descriptors: BTreeMap<String, String>,
}

impl FabricationCatalog {
    pub fn canonical() -> Self {
        let mut implementations = BTreeMap::new();
        for offer in supported_nucleus_offers() {
            let implementation = offer.implementation.implementation_id.as_str().to_owned();
            let mut prerequisites = offer
                .host_operations
                .iter()
                .map(|requirement| {
                    PrerequisiteNode::HostOperation(requirement.contract_id.as_str().to_owned())
                })
                .chain(offer.resource_requirements.iter().map(|requirement| {
                    PrerequisiteNode::Resource(requirement.class_id.as_str().to_owned())
                }))
                .collect::<Vec<_>>();
            prerequisites.sort();
            prerequisites.dedup();
            implementations
                .entry(implementation)
                .or_insert(ImplementationMetadata {
                    kind: offer.kind_id.as_str().to_owned(),
                    contract_revision: offer.kind_contract_revision.as_str().to_owned(),
                    targets: vec!["std/*/*".into()],
                    prerequisites,
                });
        }
        Self {
            implementations,
            presenters: BTreeMap::new(),
            dependencies: BTreeMap::new(),
            targets: Vec::new(),
            host_cores: Vec::new(),
            base_kinds: Vec::new(),
            base_targets: BTreeMap::new(),
            driver_kinds: Vec::new(),
            driver_targets: BTreeMap::new(),
            line_facilities: vec!["line/usb-cdc@1".into(), "line/websocket@1".into()],
            facilities: Vec::new(),
            policy_profiles: vec![
                "authority/explicit@1".into(),
                "trust/local-explicit@1".into(),
                "update/rebuild@1".into(),
            ],
            profile_fragments: vec!["profile-fragment/explicit-local-trust@1".into()],
            mutually_exclusive_mechanisms: Vec::new(),
            fabrication_descriptors: BTreeMap::new(),
        }
    }

    pub fn with_packages(mut self, packages: &FabricationPackageSet) -> Self {
        for contribution in packages.contributions() {
            self.merge_catalog_contribution(contribution.catalog());
            if let FabricationContribution::Anchor(anchor) = contribution {
                for target in &anchor.targets {
                    let key = target.key();
                    if !self.targets.contains(&key) {
                        self.targets.push(key.clone());
                    }
                    if !self.host_cores.iter().any(|item| item == &target.host_core) {
                        self.host_cores.push(target.host_core.clone());
                    }
                    for binding in &target.fabrication_descriptors {
                        self.fabrication_descriptors
                            .insert(binding.clone(), key.clone());
                    }
                }
            }
            for offer in contribution.offers() {
                if !self.base_kinds.iter().any(|item| item == &offer.base_kind) {
                    self.base_kinds.push(offer.base_kind.clone());
                }
                let base_targets = self
                    .base_targets
                    .entry(offer.base_kind.clone())
                    .or_default();
                for target in &offer.target_patterns {
                    if !base_targets.iter().any(|item| item == target) {
                        base_targets.push(target.clone());
                    }
                }
                if !self
                    .driver_kinds
                    .iter()
                    .any(|item| item == &offer.implementation_id)
                {
                    self.driver_kinds.push(offer.implementation_id.clone());
                }
                let driver_targets = self
                    .driver_targets
                    .entry(offer.implementation_id.clone())
                    .or_default();
                for target in &offer.target_patterns {
                    if !driver_targets.iter().any(|item| item == target) {
                        driver_targets.push(target.clone());
                    }
                }
            }
        }
        self.targets.sort();
        self.targets.dedup();
        self.host_cores.sort();
        self.host_cores.dedup();
        self.base_kinds.sort();
        self.base_kinds.dedup();
        self.driver_kinds.sort();
        self.driver_kinds.dedup();
        self.facilities.sort();
        self.facilities.dedup();
        self.profile_fragments.sort();
        self.profile_fragments.dedup();
        self.mutually_exclusive_mechanisms.sort();
        self.mutually_exclusive_mechanisms.dedup();
        for targets in self.base_targets.values_mut() {
            targets.sort();
            targets.dedup();
        }
        for targets in self.driver_targets.values_mut() {
            targets.sort();
            targets.dedup();
        }
        self
    }

    pub fn with_catalog_contribution(mut self, contribution: &PackageCatalogContribution) -> Self {
        self.merge_catalog_contribution(contribution);
        self.facilities.sort();
        self.facilities.dedup();
        self.profile_fragments.sort();
        self.profile_fragments.dedup();
        self.mutually_exclusive_mechanisms.sort();
        self.mutually_exclusive_mechanisms.dedup();
        self
    }

    fn merge_catalog_contribution(&mut self, contribution: &PackageCatalogContribution) {
        self.implementations
            .extend(contribution.implementations.clone());
        self.presenters.extend(contribution.presenters.clone());
        self.dependencies.extend(contribution.dependencies.clone());
        self.facilities
            .extend(contribution.facilities.iter().cloned());
        self.profile_fragments
            .extend(contribution.profile_fragments.iter().cloned());
        self.mutually_exclusive_mechanisms
            .extend(contribution.mutually_exclusive_mechanisms.iter().cloned());
    }
}
