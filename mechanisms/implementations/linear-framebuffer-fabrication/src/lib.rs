use conduit_host_fabrication::{
    FabricationContribution, FabricationExtension, HostFabricationPackage, ImplementationOffer,
    PackageCatalogContribution, PrerequisiteNode, PresenterMetadata,
};
use std::collections::BTreeMap;

pub struct LinearFramebufferFabricationExtension;

fn package_catalog() -> PackageCatalogContribution {
    PackageCatalogContribution {
        presenters: BTreeMap::from([(
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
        )]),
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
        ]),
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

impl HostFabricationPackage for LinearFramebufferFabricationExtension {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Extension(FabricationExtension {
            package_id: "conduit-linear-framebuffer@1".into(),
            package_revision: 1,
            catalog: package_catalog(),
            compatible_target_patterns: vec!["std/*/*".into(), "conduitos/x86_64/pc".into()],
            offers: vec![ImplementationOffer {
                base_kind: "display/scanout".into(),
                implementation_id: "display/linear-framebuffer@1".into(),
                implementation_revision: 1,
                target_patterns: vec!["std/x86_64/computer".into(), "conduitos/x86_64/pc".into()],
                prerequisites: Vec::new(),
                build_feature: Some("base-linear-framebuffer".into()),
            }],
        })
    }
}
