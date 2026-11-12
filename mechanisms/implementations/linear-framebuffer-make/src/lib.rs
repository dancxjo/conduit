mod initialization;
pub use initialization::CheckedScanoutInitialization;
mod native_mask_initialization;
pub use native_mask_initialization::CheckedNativeMaskInitialization;

use conduit_host_make::{
    HostMakePackage, ImplementationMetadata, ImplementationOffer, MakeContribution, MakeExtension,
    PackageCatalogContribution, PrerequisiteNode, PresenterMetadata,
};
use std::collections::BTreeMap;

pub struct LinearFramebufferMakeExtension;

fn supported_targets() -> Vec<String> {
    vec![
        "std/x86_64/computer".into(),
        "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3".into(),
        "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb".into(),
        "conduitos/x86_64/pc".into(),
        "conduitos/aarch64/virt".into(),
        "conduitos/armv6/raspberry-pi-model-b-plus-v1.2".into(),
        "conduitos/ia32/pc".into(),
        "conduitos/riscv64/virt".into(),
        "conduitos/loongarch64/virt".into(),
    ]
}

fn package_catalog() -> PackageCatalogContribution {
    let targets = supported_targets();
    let closure = conduit_conduitos_mask_offer::native_read_only_implementation_closure(
        conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
    );
    let implementations = closure
        .iter()
        .map(|offer| {
            let mut prerequisites =
                offer
                    .host_calls
                    .iter()
                    .map(|call| PrerequisiteNode::HostCall(call.contract_id.as_str().into()))
                    .chain(offer.resource_requirements.iter().map(|resource| {
                        PrerequisiteNode::Resource(resource.class_id.as_str().into())
                    }))
                    .collect::<Vec<_>>();
            if offer.kind_id.as_str() != conduit_face::FACE_TEE_KIND {
                prerequisites.push(PrerequisiteNode::Facility("compositor/native@1".into()));
            }
            (
                offer.implementation.implementation_id.as_str().into(),
                ImplementationMetadata {
                    kind: offer.kind_id.as_str().into(),
                    contract_revision: offer.kind_contract_revision.as_str().into(),
                    targets: targets.clone(),
                    prerequisites,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut presenter_roots = vec![
        PrerequisiteNode::HostCall("conduit.host/present@1".into()),
        PrerequisiteNode::Facility("compositor/native@1".into()),
        PrerequisiteNode::Resource("face/surface".into()),
        PrerequisiteNode::Resource(conduit_face::SHOW_RESOURCE_CLASS.into()),
        PrerequisiteNode::Resource(conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS.into()),
        PrerequisiteNode::Base("display/scanout".into()),
    ];
    presenter_roots.extend(
        implementations
            .keys()
            .cloned()
            .map(PrerequisiteNode::Implementation),
    );
    PackageCatalogContribution {
        implementations,
        presenters: BTreeMap::from([(
            "presenter/native-graphical@1".into(),
            PresenterMetadata {
                targets: supported_targets(),
                prerequisites: presenter_roots,
            },
        )]),
        dependencies: BTreeMap::from([
            (
                PrerequisiteNode::Facility("compositor/native@1".into()),
                vec![PrerequisiteNode::Resource("face/surface".into())],
            ),
            (
                PrerequisiteNode::Base("display/scanout".into()),
                vec![PrerequisiteNode::Driver(
                    "display/linear-framebuffer@1".into(),
                )],
            ),
        ]),
        initialization_contracts: BTreeMap::from([(
            "display/linear-framebuffer@1".into(),
            initialization::contract(),
        )]),
        facilities: vec!["compositor/native@1".into()],
        mutually_exclusive_mechanisms: vec![
            ("compositor/native@1".into(), "browser/dom".into()),
            (
                "display/linear-framebuffer@1".into(),
                "browser/dom@1".into(),
            ),
        ],
        ..Default::default()
    }
}

impl HostMakePackage for LinearFramebufferMakeExtension {
    fn contribution(&self) -> MakeContribution {
        MakeContribution::Extension(MakeExtension {
            package_id: "conduit-linear-framebuffer@1".into(),
            package_revision: 3,
            catalog: package_catalog(),
            compatible_target_patterns: vec![
                "std/*/*".into(),
                "conduitos/x86_64/pc".into(),
                "conduitos/aarch64/virt".into(),
                "conduitos/armv6/raspberry-pi-model-b-plus-v1.2".into(),
                "conduitos/ia32/pc".into(),
                "conduitos/riscv64/virt".into(),
                "conduitos/loongarch64/virt".into(),
            ],
            offers: vec![ImplementationOffer {
                base_kind: "display/scanout".into(),
                implementation_id: "display/linear-framebuffer@1".into(),
                implementation_revision: 1,
                target_patterns: supported_targets(),
                prerequisites: Vec::new(),
                build_feature: Some("base-linear-framebuffer".into()),
            }],
        })
    }
}
