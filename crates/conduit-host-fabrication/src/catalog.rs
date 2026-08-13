use std::collections::BTreeMap;

use conduit_std_catalog::supported_nucleus_offers;

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
            ]),
            targets: vec![
                "std/x86_64/workstation".into(),
                "std/x86_64/server".into(),
                "browser/wasm32/page".into(),
                "conduitos/x86_64/pc".into(),
                "conduitos/aarch64/virt".into(),
                "conduitos/thumbv6m/pico-w".into(),
            ],
            host_cores: vec!["host-core/std@1".into(), "host-core/conduitos@1".into()],
            base_kinds: vec![
                "browser/dom".into(),
                "clock/monotonic".into(),
                "display/scanout".into(),
                "serial/text".into(),
                "storage/protected-file".into(),
                "timer/monotonic".into(),
            ],
            base_targets: BTreeMap::from([
                ("browser/dom".into(), vec!["browser/wasm32/page".into()]),
                ("clock/monotonic".into(), vec!["std/*/*".into()]),
                (
                    "display/scanout".into(),
                    vec![
                        "std/x86_64/workstation".into(),
                        "conduitos/x86_64/pc".into(),
                    ],
                ),
                (
                    "serial/text".into(),
                    vec!["std/*/*".into(), "conduitos/*/*".into()],
                ),
                ("storage/protected-file".into(), vec!["std/*/*".into()]),
                ("timer/monotonic".into(), vec!["std/*/*".into()]),
            ]),
            driver_kinds: vec![
                "browser/dom@1".into(),
                "conduitos/pl011@1".into(),
                "display/linear-framebuffer@1".into(),
                "hosted/monotonic-clock@1".into(),
                "hosted/protected-file@1".into(),
                "hosted/serial@1".into(),
                "pico/usb-cdc@1".into(),
            ],
            driver_targets: BTreeMap::from([
                ("browser/dom@1".into(), vec!["browser/wasm32/page".into()]),
                (
                    "conduitos/pl011@1".into(),
                    vec!["conduitos/aarch64/virt".into()],
                ),
                (
                    "display/linear-framebuffer@1".into(),
                    vec![
                        "std/x86_64/workstation".into(),
                        "conduitos/x86_64/pc".into(),
                    ],
                ),
                ("hosted/monotonic-clock@1".into(), vec!["std/*/*".into()]),
                ("hosted/protected-file@1".into(), vec!["std/*/*".into()]),
                ("hosted/serial@1".into(), vec!["std/*/*".into()]),
                (
                    "pico/usb-cdc@1".into(),
                    vec!["conduitos/thumbv6m/pico-w".into()],
                ),
            ]),
            line_facilities: vec!["line/usb-cdc@1".into(), "line/websocket@1".into()],
            facilities: vec!["compositor/native@1".into()],
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
        }
    }
}
