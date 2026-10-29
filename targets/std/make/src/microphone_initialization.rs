//! Declarative initialization contract; never probes or opens a capture stream.
use conduit_host_make::{
    BaseInitializationContract, ConfigurationValue, ImplementationOffer, KindConfigurationField,
    KindConfigurationRule,
};

pub const ALSA_MICROPHONE_BASE_KIND: &str = "audio/microphone";

pub fn alsa_microphone_initialization_contract() -> BaseInitializationContract {
    let text = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text("".into()),
        rule: KindConfigurationRule::DataLabel { maximum },
    };
    let count = |key: &str, minimum, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(minimum),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    };
    BaseInitializationContract {
        fields: vec![
            text("executable", 4096),
            text("executable_sha256", 64),
            text("card_id", 64),
            text("base_identity", 256),
            count("device", 0, 65535),
            count("capture_milliseconds", 1, 6000),
            count("timeout_milliseconds", 1, 120000),
        ],
    }
}

pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: ALSA_MICROPHONE_BASE_KIND.into(),
        implementation_id: conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION.into(),
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

pub(super) fn converted_offer() -> ImplementationOffer {
    let mut offer = offer();
    offer.implementation_id = conduit_std_offers::CONVERTED_MICROPHONE_CLIP_IMPLEMENTATION.into();
    offer
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn microphone_contract_is_finite_and_platform_support_explicit() {
        alsa_microphone_initialization_contract()
            .validate()
            .unwrap();
        let offer = offer();
        assert!(offer
            .target_patterns
            .contains(&"std/x86_64/computer".into()));
        assert!(!offer
            .target_patterns
            .iter()
            .any(|p| p.contains("macos") || p.contains("windows")));
    }
}

#[cfg(test)]
mod source_tests {
    use conduit_host_make::{
        canonical_host_configuration_conduit, check_host_configuration,
        parse_host_configuration_conduit, MakeCatalog, MakePackageSet,
    };
    fn source() -> String {
        include_str!("../../profiles/linux-computer.host.conduit").replace("  limits =", r#"  base = {kind: "audio/microphone", implementation: "std/alsa-microphone-clip@1", initialization: {executable: "/usr/bin/arecord", executable_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", card_id: "CMTECK", base_identity: "fixture/usb-microphone", device: 0, capture_milliseconds: 3000, timeout_milliseconds: 10000}}
  limits ="#)
    }
    fn checked(
        source: &str,
    ) -> Result<
        conduit_host_make::CheckedHostConfiguration,
        Vec<conduit_host_make::ConfigurationDiagnostic>,
    > {
        let packages = MakePackageSet::compose(&[&crate::HostedMakePackage]).unwrap();
        check_host_configuration(
            parse_host_configuration_conduit(source).unwrap(),
            &MakeCatalog::canonical().with_packages(&packages),
            &packages,
        )
    }
    #[test]
    fn microphone_source_round_trips_and_initialization_changes_exact_identity() {
        let first = checked(&source()).unwrap();
        let encoded = canonical_host_configuration_conduit(first.configuration()).unwrap();
        let round_trip = checked(&encoded).unwrap();
        assert_eq!(first.configuration_id(), round_trip.configuration_id());
        assert_eq!(
            encoded,
            canonical_host_configuration_conduit(round_trip.configuration()).unwrap()
        );
        let changed =
            checked(&source().replace("capture_milliseconds: 3000", "capture_milliseconds: 2000"))
                .unwrap();
        assert_ne!(first.configuration_id(), changed.configuration_id());
        assert_ne!(
            conduit_host_make::canonical_profile_json(first.profile()).unwrap(),
            conduit_host_make::canonical_profile_json(changed.profile()).unwrap()
        );
    }
    #[test]
    fn converted_implementation_is_explicit_and_changes_checked_identity() {
        let direct = checked(&source()).unwrap();
        let converted = checked(&source().replace(
            conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION,
            conduit_std_offers::CONVERTED_MICROPHONE_CLIP_IMPLEMENTATION,
        ))
        .unwrap();
        assert_ne!(direct.configuration_id(), converted.configuration_id());
        assert!(converted
            .resolved_bases()
            .iter()
            .any(|(_, implementation)| implementation
                == conduit_std_offers::CONVERTED_MICROPHONE_CLIP_IMPLEMENTATION));
        let canonical = canonical_host_configuration_conduit(direct.configuration()).unwrap();
        assert_eq!(
            direct.configuration_id(),
            checked(&canonical).unwrap().configuration_id()
        );
    }
    #[test]
    fn microphone_initialization_has_no_implicit_defaults_and_cannot_target_windows() {
        for source in [
            source().replace("device: 0, ", ""),
            source().replace("device: 0", "unexpected: 0"),
            source().replace("capture_milliseconds: 3000", "capture_milliseconds: 7000"),
            source().replace(
                "machine: \"computer\", os: \"linux\"",
                "machine: \"windows-computer\", os: \"windows\"",
            ),
        ] {
            assert!(checked(&source).is_err());
        }
    }
    #[test]
    fn duplicate_base_initializations_must_agree() {
        let source = source();
        let microphone = source
            .lines()
            .find(|line| line.contains("audio/microphone"))
            .unwrap();
        let duplicate = source.replace(
            "  limits =",
            &format!(
                "{}\n  limits =",
                microphone.replace("capture_milliseconds: 3000", "capture_milliseconds: 2000")
            ),
        );
        assert!(checked(&duplicate).is_err());
    }
}
