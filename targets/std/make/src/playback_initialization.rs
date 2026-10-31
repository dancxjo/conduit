//! Checked finite speaker intent; metadata observation remains a Boot effect.
use conduit_host_make::{
    BaseInitializationContract, ConfigurationValue, ImplementationOffer, KindConfigurationField,
    KindConfigurationRule,
};
pub(super) fn contract() -> BaseInitializationContract {
    let text = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text("".into()),
        rule: KindConfigurationRule::DataLabel { maximum },
    };
    BaseInitializationContract {
        fields: vec![
            text("executable_sha256", 64),
            text("card_id", 64),
            text("base_identity", 256),
            KindConfigurationField {
                key: "device".into(),
                default_value: ConfigurationValue::U64(0),
                rule: KindConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: 65535,
                },
            },
        ],
    }
}
pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: "audio/speaker".into(),
        implementation_id: conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION.into(),
        implementation_revision: 1,
        target_patterns: vec![
            "std/x86_64/computer".into(),
            "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3".into(),
            "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb".into(),
        ],
        prerequisites: Vec::new(),
        build_feature: None,
    }
}
#[cfg(test)]
mod tests {
    use conduit_host_make::*;
    fn checked(source: &str) -> Result<CheckedHostConfiguration, Vec<ConfigurationDiagnostic>> {
        let packages = MakePackageSet::compose(&[&crate::HostedMakePackage]).unwrap();
        check_host_configuration(
            parse_host_configuration_conduit(source).unwrap(),
            &MakeCatalog::canonical().with_packages(&packages),
            &packages,
        )
    }
    fn source() -> String {
        include_str!("../../profiles/linux-computer.host.conduit").replace("  limits =",r#"  base = {kind: "audio/speaker", implementation: "std/kernel-audio-play-alsa-hw@1", initialization: {executable_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", card_id: "Headphones", device: 0, base_identity: "bcm2835-audio"}}
  limits ="#)
    }
    #[test]
    fn speaker_checked_identity_round_trips_and_refuses_unsupported_or_unbounded_intent() {
        let first = checked(&source()).unwrap();
        let encoded = canonical_host_configuration_conduit(first.configuration()).unwrap();
        assert_eq!(
            first.configuration_id(),
            checked(&encoded).unwrap().configuration_id()
        );
        assert_ne!(
            first.configuration_id(),
            checked(&source().replace("Headphones", "Other"))
                .unwrap()
                .configuration_id()
        );
        for invalid in [
            source().replace("device: 0, ", ""),
            source().replace("device: 0", "device: 65536"),
            source().replace("device: 0", "unexpected: 0"),
            source().replace(
                r#"machine: "computer", os: "linux""#,
                r#"machine: "windows-computer", os: "windows""#,
            ),
        ] {
            assert!(checked(&invalid).is_err());
        }
    }
}
