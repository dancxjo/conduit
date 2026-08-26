use conduit_host_fabrication::{
    FabricationContribution, FabricationExtension, HostFabricationPackage, ImplementationOffer,
};

pub struct LinearFramebufferFabricationExtension;

impl HostFabricationPackage for LinearFramebufferFabricationExtension {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Extension(FabricationExtension {
            package_id: "conduit-linear-framebuffer@1".into(),
            package_revision: 1,
            compatible_target_patterns: vec!["std/*/*".into(), "conduitos/x86_64/pc".into()],
            offers: vec![ImplementationOffer {
                base_kind: "display/scanout".into(),
                implementation_id: "display/linear-framebuffer@1".into(),
                implementation_revision: 1,
                target_patterns: vec![
                    "std/x86_64/workstation".into(),
                    "conduitos/x86_64/pc".into(),
                ],
                prerequisites: Vec::new(),
                build_feature: Some("base-linear-framebuffer".into()),
            }],
        })
    }
}
