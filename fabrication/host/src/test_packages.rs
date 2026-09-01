use crate::{
    FabricationAnchor, FabricationCatalog, FabricationContribution, FabricationExtension,
    FabricationPackageSet, HostBounds, ImplementationMetadata, ImplementationOffer,
    PackageCatalogContribution, PostBuildAction, PrerequisiteNode, PresenterMetadata,
    SporeOutputKind, TargetDescriptor, TargetPresenter,
};
use std::collections::BTreeMap;

fn maxima(memory: u64, items: u32) -> HostBounds {
    HostBounds {
        static_memory_bytes: memory,
        heap_arena_bytes: memory,
        queue_items: items,
        buffered_bytes: memory,
        active_instances: items,
        operation_slots: items,
        timer_slots: items,
        line_sessions: items,
        evidence_items: items,
    }
}

#[allow(clippy::too_many_arguments)]
fn target(
    label: &str,
    family: &str,
    architecture: &str,
    machine: &str,
    board: Option<&str>,
    os: Option<&str>,
    host_core: &str,
    maxima: HostBounds,
) -> TargetDescriptor {
    let (toolchain, builder, deployment, outputs, actions) = match family {
        "std" => (
            "rustc:stable",
            "hosted-native/build@1",
            Some("conduit.deploy/native-directory@1"),
            vec![SporeOutputKind::NativeBundle],
            vec![PostBuildAction::Launch],
        ),
        "browser" => (
            "rustc:stable+wasm32-unknown-unknown",
            "browser-wasm/build@1",
            Some("conduit.deploy/browser-directory@1"),
            vec![SporeOutputKind::BrowserBundle],
            vec![PostBuildAction::Load, PostBuildAction::Launch],
        ),
        "esp32" => (
            "esp-rs/rust-build@v1.91.1.0",
            "conduit-host-esp32/build-image@1",
            None,
            vec![SporeOutputKind::Esp32Image],
            vec![PostBuildAction::Flash, PostBuildAction::Boot],
        ),
        _ if architecture == "thumbv6m" => (
            "rustc:stable+thumbv6m-none-eabi",
            "conduit-host-rp2040/build-uf2@1",
            Some("conduit-host-rp2040/flash-uf2@1"),
            vec![SporeOutputKind::Uf2],
            vec![PostBuildAction::Flash, PostBuildAction::Boot],
        ),
        _ => (
            "rustc:stable+llvm-tools",
            "conduitos/build-image@1",
            None,
            vec![SporeOutputKind::DiskImage, SporeOutputKind::EfiArtifact],
            vec![PostBuildAction::Boot],
        ),
    };
    TargetDescriptor {
        label: label.into(),
        family: family.into(),
        architecture: architecture.into(),
        machine: machine.into(),
        board: board.map(Into::into),
        os: os.map(Into::into),
        host_core: host_core.into(),
        presenter: (family == "conduitos" && architecture == "aarch64").then(|| TargetPresenter {
            id: "presenter/main".into(),
            implementation_id: "presenter/linear-serial@1".into(),
            interactive: false,
        }),
        host_operations: (family == "conduitos" && architecture == "aarch64")
            .then(|| "conduit.host/present@1".into())
            .into_iter()
            .collect(),
        toolchain_identity: toolchain.into(),
        builder_adapter: builder.into(),
        deployment_adapter: deployment.map(Into::into),
        outputs,
        default_output: match family {
            "std" => SporeOutputKind::NativeBundle,
            "browser" => SporeOutputKind::BrowserBundle,
            "esp32" => SporeOutputKind::Esp32Image,
            _ if architecture == "thumbv6m" => SporeOutputKind::Uf2,
            _ => SporeOutputKind::DiskImage,
        },
        post_build_actions: actions,
        fabrication_descriptors: Vec::new(),
        maxima,
    }
}

fn offer(kind: &str, implementation: &str, feature: &str, pattern: &str) -> ImplementationOffer {
    ImplementationOffer {
        base_kind: kind.into(),
        implementation_id: implementation.into(),
        implementation_revision: 1,
        target_patterns: vec![pattern.into()],
        prerequisites: Vec::new(),
        build_feature: Some(feature.into()),
    }
}

pub(crate) fn test_package_set() -> FabricationPackageSet {
    let hosted_maxima = maxima(2 * 1024 * 1024 * 1024, 1_048_576);
    FabricationPackageSet::from_contributions([
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "hosted-native@1".into(),
            package_revision: 1,
            catalog: Default::default(),
            targets: vec![target(
                "Hosted computer · Linux · x86_64",
                "std",
                "x86_64",
                "computer",
                None,
                Some("linux"),
                "host-core/std@1",
                hosted_maxima.clone(),
            )],
            offers: vec![
                offer(
                    "clock/monotonic",
                    "hosted/monotonic-clock@1",
                    "base-clock",
                    "std/*/*",
                ),
                offer("serial/text", "hosted/serial@1", "base-serial", "std/*/*"),
                offer(
                    "storage/protected-file",
                    "hosted/protected-file@1",
                    "base-protected-file",
                    "std/*/*",
                ),
                offer(
                    "timer/monotonic",
                    "hosted/monotonic-clock@1",
                    "base-timer",
                    "std/*/*",
                ),
            ],
        }),
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "browser-wasm@1".into(),
            package_revision: 1,
            catalog: Default::default(),
            targets: vec![target(
                "Browser page",
                "browser",
                "wasm32",
                "page",
                None,
                None,
                "host-core/std@1",
                hosted_maxima,
            )],
            offers: vec![offer(
                "browser/dom",
                "browser/dom@1",
                "base-browser-dom",
                "browser/wasm32/page",
            )],
        }),
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "conduit-host-rp2040@1".into(),
            package_revision: 1,
            catalog: Default::default(),
            targets: vec![target(
                "Pico W",
                "conduitos",
                "thumbv6m",
                "pico-w",
                Some("pico-w"),
                None,
                "host-core/conduitos@1",
                maxima(256 * 1024, 4096),
            )],
            offers: vec![offer(
                "serial/text",
                "pico/usb-cdc@1",
                "line-usb-cdc",
                "conduitos/thumbv6m/pico-w",
            )],
        }),
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "conduit-host-esp32@1".into(),
            package_revision: 1,
            catalog: Default::default(),
            targets: vec![
                target(
                    "ESP32 WROOM",
                    "esp32",
                    "xtensa-lx6",
                    "hw-463-esp-wroom-32",
                    Some("hw-463-esp-wroom-32"),
                    None,
                    "host-core/conduitos@1",
                    maxima(64 * 1024 * 1024, 4096),
                ),
                target(
                    "ESP32 fixture",
                    "esp32",
                    "xtensa",
                    "esp-wroom-32",
                    None,
                    None,
                    "host-core/conduitos@1",
                    maxima(64 * 1024 * 1024, 4096),
                ),
            ],
            offers: vec![
                offer(
                    "kernel/signal",
                    "esp32/kernel-signal@1",
                    "kernel-signal",
                    "esp32/xtensa-lx6/*",
                ),
                offer(
                    "line/bluetooth-le-gatt",
                    "esp32/bluetooth-le-gatt@1",
                    "bluetooth",
                    "esp32/xtensa-lx6/*",
                ),
            ],
        }),
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "conduitos-image@1".into(),
            package_revision: 1,
            catalog: test_catalog_metadata(),
            targets: vec![
                target(
                    "ConduitOS x86_64 PC",
                    "conduitos",
                    "x86_64",
                    "pc",
                    None,
                    None,
                    "host-core/conduitos@1",
                    maxima(512 * 1024 * 1024, 65_536),
                ),
                target(
                    "ConduitOS aarch64 virt",
                    "conduitos",
                    "aarch64",
                    "virt",
                    None,
                    None,
                    "host-core/conduitos@1",
                    maxima(512 * 1024 * 1024, 65_536),
                ),
            ],
            offers: vec![offer(
                "serial/text",
                "conduitos/pl011@1",
                "base-pl011",
                "conduitos/aarch64/virt",
            )],
        }),
        FabricationContribution::Extension(FabricationExtension {
            package_id: "linear-framebuffer-fixture@1".into(),
            package_revision: 1,
            catalog: Default::default(),
            compatible_target_patterns: vec!["std/*/*".into(), "conduitos/x86_64/pc".into()],
            offers: vec![ImplementationOffer {
                base_kind: "display/scanout".into(),
                implementation_id: "display/linear-framebuffer@1".into(),
                implementation_revision: 1,
                target_patterns: vec!["std/x86_64/computer".into(), "conduitos/x86_64/pc".into()],
                prerequisites: Vec::new(),
                build_feature: Some("base-linear-framebuffer".into()),
            }],
        }),
    ])
    .expect("test fabrication packages are valid")
}

pub(crate) fn test_catalog() -> FabricationCatalog {
    FabricationCatalog::canonical()
        .with_packages(&test_package_set())
        .with_catalog_contribution(&test_catalog_metadata())
}

fn test_catalog_metadata() -> PackageCatalogContribution {
    let mut implementations = conduit_std_offers::supported_nucleus_offers()
        .into_iter()
        .map(|offer| {
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
            (
                implementation,
                ImplementationMetadata {
                    kind: offer.kind_id.as_str().to_owned(),
                    contract_revision: offer.kind_contract_revision.as_str().to_owned(),
                    targets: vec!["std/*/*".into()],
                    prerequisites,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    implementations.insert(
        "conduitos/kernel-http-client-http1-literal@1".into(),
        ImplementationMetadata {
            kind: "http/client".into(),
            contract_revision: "conduit.http/client@1".into(),
            targets: vec!["conduitos/x86_64/pc".into()],
            prerequisites: vec![
                PrerequisiteNode::HostOperation("conduit.host/http-client-exchange@1".into()),
                PrerequisiteNode::Resource("conduit.resource/network/http-client@1".into()),
                PrerequisiteNode::Facility("network/http1-literal-client@1".into()),
            ],
        },
    );
    PackageCatalogContribution {
        implementations,
        presenters: BTreeMap::from([
            (
                "presenter/native-graphical@1".into(),
                PresenterMetadata {
                    targets: vec!["std/x86_64/computer".into(), "conduitos/x86_64/pc".into()],
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
        facilities: vec![
            "compositor/native@1".into(),
            "network/http1-literal-client@1".into(),
        ],
        profile_fragments: vec![
            "profile-fragment/conduitos-scripted-keyboard-proof@1".into(),
            "profile-fragment/conduitos-hotplug-proof@1".into(),
        ],
        mutually_exclusive_mechanisms: vec![
            ("compositor/native@1".into(), "browser/dom".into()),
            (
                "display/linear-framebuffer@1".into(),
                "browser/dom@1".into(),
            ),
        ],
    }
}

pub(crate) fn test_build_host_image(
    profile: crate::HostProfile,
    catalog: &FabricationCatalog,
    inputs: &crate::BuildInputs,
) -> Result<(crate::HostImage, Vec<u8>), Vec<crate::BuildDiagnostic>> {
    let output = match profile.target.family.as_str() {
        "std" => SporeOutputKind::NativeBundle,
        "browser" => SporeOutputKind::BrowserBundle,
        "esp32" => SporeOutputKind::Esp32Image,
        "conduitos" if profile.target.architecture == "thumbv6m" => SporeOutputKind::Uf2,
        "conduitos" => SporeOutputKind::DiskImage,
        _ => panic!("test fixture has no output for {}", profile.target.key()),
    };
    crate::build_host_image(profile, catalog, &test_package_set(), &output, inputs)
}
