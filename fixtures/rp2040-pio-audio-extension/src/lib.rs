use conduit_host_fabrication::{
    FabricationContribution, FabricationExtension, HostFabricationPackage, ImplementationOffer,
};

pub const PACKAGE_ID: &str = "example-rp2040-pio-audio@1";
pub const IMPLEMENTATION_ID: &str = "example/rp2040-pio-audio@1";

pub struct Rp2040PioAudioExtension;

impl HostFabricationPackage for Rp2040PioAudioExtension {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Extension(FabricationExtension {
            package_id: PACKAGE_ID.into(),
            package_revision: 1,
            compatible_target_patterns: vec!["conduitos/thumbv6m/*".into()],
            offers: vec![ImplementationOffer {
                base_kind: "audio/pcm-output".into(),
                implementation_id: IMPLEMENTATION_ID.into(),
                implementation_revision: 1,
                target_patterns: vec!["conduitos/thumbv6m/pico-w".into()],
                prerequisites: vec!["resource/rp2040-pio-state-machine@1".into()],
                build_feature: Some("base-pio-audio".into()),
            }],
        })
    }
}
