use std::collections::BTreeMap;

use conduit_std_catalog::supported_nucleus_offers;

use crate::{Esp32BoardDescriptor, FabricationContribution, FabricationPackageSet};

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
    pub esp32_descriptors: BTreeMap<String, Esp32BoardDescriptor>,
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
        implementations.insert(
            conduitos_http_implementation().into(),
            ImplementationMetadata {
                kind: conduit_std_catalog::HTTP_CLIENT_KIND.into(),
                contract_revision: conduit_std_catalog::HTTP_CLIENT_REVISION.into(),
                targets: vec!["conduitos/x86_64/pc".into()],
                prerequisites: vec![
                    PrerequisiteNode::HostOperation("conduit.host/http-client-exchange@1".into()),
                    PrerequisiteNode::Resource("conduit.resource/network/http-client@1".into()),
                    PrerequisiteNode::Facility("network/http1-literal-client@1".into()),
                ],
            },
        );
        Self {
            implementations,
            presenters: BTreeMap::from([
                (
                    "presenter/native-graphical@1".into(),
                    PresenterMetadata {
                        targets: vec![
                            "std/x86_64/workstation".into(),
                            "conduitos/x86_64/pc".into(),
                        ],
                        prerequisites: vec![
                            PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                            PrerequisiteNode::Facility("compositor/native@1".into()),
                            PrerequisiteNode::Resource("presentation/surface".into()),
                            PrerequisiteNode::Base("display/scanout".into()),
                        ],
                    },
                ),
                (
                    "presenter/browser-dom-svg@1".into(),
                    PresenterMetadata {
                        targets: vec!["browser/wasm32/page".into()],
                        prerequisites: vec![
                            PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                            PrerequisiteNode::Resource("presentation/surface".into()),
                            PrerequisiteNode::Base("browser/dom".into()),
                        ],
                    },
                ),
                (
                    "presenter/linear-serial@1".into(),
                    PresenterMetadata {
                        targets: vec!["conduitos/aarch64/virt".into()],
                        prerequisites: vec![
                            PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                            PrerequisiteNode::Base("serial/text".into()),
                        ],
                    },
                ),
            ]),
            dependencies: BTreeMap::from([
                (
                    PrerequisiteNode::Facility("compositor/native@1".into()),
                    vec![PrerequisiteNode::Resource("presentation/surface".into())],
                ),
                (
                    PrerequisiteNode::Base("display/scanout".into()),
                    vec![PrerequisiteNode::Driver(
                        "display/linear-framebuffer@1".into(),
                    )],
                ),
                (
                    PrerequisiteNode::Resource("conduit.resource/timer-slot@1".into()),
                    vec![PrerequisiteNode::Base("timer/monotonic".into())],
                ),
                (
                    PrerequisiteNode::Base("timer/monotonic".into()),
                    vec![PrerequisiteNode::Driver("hosted/monotonic-clock@1".into())],
                ),
                (
                    PrerequisiteNode::Base("serial/text".into()),
                    vec![PrerequisiteNode::Driver("conduitos/pl011@1".into())],
                ),
                (
                    PrerequisiteNode::Facility("network/http1-literal-client@1".into()),
                    vec![
                        PrerequisiteNode::Resource("network/packet-buffer@1".into()),
                        PrerequisiteNode::Resource("network/tcp-socket@1".into()),
                        PrerequisiteNode::Resource("network/timer@1".into()),
                        PrerequisiteNode::Base("network/ipv4-tcp".into()),
                    ],
                ),
                (
                    PrerequisiteNode::Base("network/ipv4-tcp".into()),
                    vec![PrerequisiteNode::Driver(
                        "conduitos/deterministic-ipv4-tcp@1".into(),
                    )],
                ),
            ]),
            targets: Vec::new(),
            host_cores: Vec::new(),
            base_kinds: Vec::new(),
            base_targets: BTreeMap::new(),
            driver_kinds: Vec::new(),
            driver_targets: BTreeMap::new(),
            line_facilities: vec!["line/usb-cdc@1".into(), "line/websocket@1".into()],
            facilities: vec![
                "compositor/native@1".into(),
                "network/http1-literal-client@1".into(),
            ],
            policy_profiles: vec![
                "authority/explicit@1".into(),
                "trust/local-explicit@1".into(),
                "update/rebuild@1".into(),
            ],
            profile_fragments: vec![
                "profile-fragment/explicit-local-trust@1".into(),
                "profile-fragment/conduitos-scripted-keyboard-proof@1".into(),
                "profile-fragment/conduitos-hotplug-proof@1".into(),
            ],
            esp32_descriptors: BTreeMap::new(),
        }
    }

    pub fn with_packages(mut self, packages: &FabricationPackageSet) -> Self {
        for contribution in packages.contributions() {
            if let FabricationContribution::Anchor(anchor) = contribution {
                for target in &anchor.targets {
                    let key = target.key();
                    if !self.targets.contains(&key) {
                        self.targets.push(key);
                    }
                    if !self.host_cores.iter().any(|item| item == &target.host_core) {
                        self.host_cores.push(target.host_core.clone());
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
}

const fn conduitos_http_implementation() -> &'static str {
    "conduitos/kernel-http-client-http1-literal@1"
}
