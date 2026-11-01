//! Finite environmental equipment intent. Checking performs no discovery,
//! opens no device or socket, and publishes no native runtime authority.
use conduit_host_make::{
    BaseInitializationContract, ConfigurationValue, ImplementationOffer, KindConfigurationField,
    KindConfigurationRule,
};

pub const ENVIRONMENTAL_SOURCE_BASE_KIND: &str = "environment/source";
pub const ENVIRONMENTAL_SOURCES_IMPLEMENTATION: &str = "linux/environmental-sources@1";
/// Serial and Network attachments select this adapter; per-device parameters
/// are source_configuration and firmware_sha256. Root validates the absolute
/// configuration path and exact lowercase SHA256 before acquiring equipment.
pub const ENVIRONMENTAL_SOURCE_ATTACHMENT_IMPLEMENTATION: &str = "environment/source@1";

pub fn environmental_source_initialization_contract() -> BaseInitializationContract {
    let text = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text("".into()),
        rule: KindConfigurationRule::DataLabel { maximum },
    };
    BaseInitializationContract {
        fields: vec![
            text("worker_executable", 4096),
            text("worker_sha256", 64),
            KindConfigurationField {
                key: "timeout_milliseconds".into(),
                default_value: ConfigurationValue::U64(31_000),
                rule: KindConfigurationRule::U64Range {
                    minimum: 31_000,
                    maximum: 45_000,
                },
            },
        ],
    }
}

pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: ENVIRONMENTAL_SOURCE_BASE_KIND.into(),
        implementation_id: ENVIRONMENTAL_SOURCES_IMPLEMENTATION.into(),
        implementation_revision: 1,
        target_patterns: vec![
            "std/x86_64/computer".into(),
            "std/armv6/raspberry-pi-model-b-plus-v1.2".into(),
            "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3".into(),
            "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb".into(),
            "std/aarch64/raspberry-pi-zero-2-w-rev-1.0".into(),
            "std/aarch64/raspberry-pi-zero-2-wh-rev-1.0".into(),
            "std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb".into(),
        ],
        prerequisites: Vec::new(),
        build_feature: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_make::{
        canonical_host_configuration_conduit, check_host_configuration,
        parse_host_configuration_conduit, CheckedHostConfiguration, ConfigurationDiagnostic,
        MakeCatalog, MakePackageSet,
    };
    fn source() -> String {
        include_str!("../../profiles/linux-computer.host.conduit").replace("  limits =", r#"  base = {kind: "environment/source", implementation: "linux/environmental-sources@1", initialization: {worker_executable: "/opt/conduit/environment-http-provider", worker_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", timeout_milliseconds: 31000}}
  device = {kind: "serial", id: "usb", path: "/dev/serial/by-id/fixture", implementation: "environment/source@1", configuration: {source_configuration: "/opt/conduit/sensor.host.conduit", firmware_sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}}
  device = {kind: "network", id: "remote", authority: "sensor.local:8767", endpoint: "192.0.2.1:8767", implementation: "environment/source@1", configuration: {source_configuration: "/opt/conduit/remote.host.conduit", firmware_sha256: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}}
  limits ="#)
    }
    fn checked(source: &str) -> Result<CheckedHostConfiguration, Vec<ConfigurationDiagnostic>> {
        let packages = MakePackageSet::compose(&[&crate::HostedMakePackage]).unwrap();
        check_host_configuration(
            parse_host_configuration_conduit(source).unwrap(),
            &MakeCatalog::canonical().with_packages(&packages),
            &packages,
        )
    }
    #[test]
    fn environmental_equipment_roundtrips_and_changes_checked_identity() {
        environmental_source_initialization_contract()
            .validate()
            .unwrap();
        let original = checked(&source()).unwrap();
        let encoded = canonical_host_configuration_conduit(original.configuration()).unwrap();
        let recovered = checked(&encoded).unwrap();
        assert_eq!(original.configuration_id(), recovered.configuration_id());
        assert_eq!(
            encoded,
            canonical_host_configuration_conduit(recovered.configuration()).unwrap()
        );
        assert!(original.resolved_bases().contains(&(
            ENVIRONMENTAL_SOURCE_BASE_KIND.into(),
            ENVIRONMENTAL_SOURCES_IMPLEMENTATION.into()
        )));
        let changed = checked(
            &source().replace("timeout_milliseconds: 31000", "timeout_milliseconds: 45000"),
        )
        .unwrap();
        assert_ne!(original.configuration_id(), changed.configuration_id());
    }
    #[test]
    fn environmental_equipment_refuses_missing_unknown_and_out_of_range_initialization() {
        for (before, after) in [
            ("timeout_milliseconds: 31000", "timeout_milliseconds: 30999"),
            ("timeout_milliseconds: 31000", "timeout_milliseconds: 45001"),
            ("timeout_milliseconds: 31000", "timeout_milliseconds: true"),
            (", timeout_milliseconds: 31000", ""),
            ("timeout_milliseconds: 31000", "unknown: 31000"),
            (
                "worker_executable: \"/opt/conduit/environment-http-provider\"",
                "worker_executable: true",
            ),
        ] {
            assert!(
                checked(&source().replace(before, after)).is_err(),
                "{after}"
            );
        }
        let oversized = "a".repeat(65);
        assert!(checked(&source().replace(&"a".repeat(64), &oversized)).is_err());
    }
    #[test]
    fn environmental_offer_targets_only_explicit_linux_hosts() {
        let offer = offer();
        assert_eq!(offer.target_patterns.len(), 7);
        assert!(offer
            .target_patterns
            .contains(&crate::HOSTED_TARGET_ID.into()));
        assert!(offer
            .target_patterns
            .iter()
            .all(|target| target.starts_with("std/")
                && !target.contains('*')
                && !target.contains("windows")
                && !target.contains("macos")));
        for (architecture, machine, os) in [
            ("x86_64", "windows-computer", "windows"),
            ("aarch64", "macos-computer", "macos"),
        ] {
            let invalid = source()
                .replace(
                    "architecture: \"x86_64\"",
                    &format!("architecture: \"{architecture}\""),
                )
                .replace("machine: \"computer\"", &format!("machine: \"{machine}\""))
                .replace("os: \"linux\"", &format!("os: \"{os}\""));
            assert!(checked(&invalid).is_err(), "{os}");
        }
    }
}
