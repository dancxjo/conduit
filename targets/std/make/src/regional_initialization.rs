//! Finite local viewer preferences. Checking the declaration never reads the OS.
use conduit_host_make::{
    BaseInitializationContract, ConfigurationValue, ImplementationOffer, KindConfigurationField,
    KindConfigurationRule,
};

pub(super) const IMPLEMENTATION: &str = "hosted/linux-regional-preferences@1";

pub(super) fn contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![KindConfigurationField {
            key: "temperature".into(),
            default_value: ConfigurationValue::Text("system".into()),
            rule: KindConfigurationRule::TextOneOf {
                values: ["system", "celsius", "fahrenheit"]
                    .map(String::from)
                    .to_vec(),
            },
        }],
    }
}

pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: "locale/regional-preferences".into(),
        implementation_id: IMPLEMENTATION.into(),
        implementation_revision: 1,
        target_patterns: [
            "std/x86_64/computer",
            "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3",
            "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb",
        ]
        .map(String::from)
        .to_vec(),
        prerequisites: Vec::new(),
        build_feature: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_make::*;
    #[test]
    fn checked_linux_source_selects_finite_temperature_preference() {
        let packages = MakePackageSet::compose(&[&crate::HostedMakePackage]).unwrap();
        let catalog = MakeCatalog::canonical().with_packages(&packages);
        let source = include_str!("../../profiles/linux-computer.host.conduit").replace("  limits =", r#"  base = {kind: "locale/regional-preferences", implementation: "hosted/linux-regional-preferences@1", initialization: {temperature: "fahrenheit"}}
  limits ="#);
        let checked = |source: &str| {
            check_host_configuration(
                parse_host_configuration_conduit(source).unwrap(),
                &catalog,
                &packages,
            )
        };
        let with_override = checked(&source).unwrap();
        assert!(with_override
            .resolved_bases()
            .iter()
            .any(|(_, id)| id == IMPLEMENTATION));
        assert_ne!(
            with_override.configuration_id(),
            checked(&source.replace("fahrenheit", "system"))
                .unwrap()
                .configuration_id()
        );
        assert!(checked(&source.replace("fahrenheit", "guess")).is_err());
    }
}
