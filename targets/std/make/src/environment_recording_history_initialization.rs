//! Explicit bounded recording selection; this contract performs no IO or grant.
//! Checked input supplies every field explicitly. The Source retains the entire
//! latest original generation; no tail selection or generic Data budget changes.
use conduit_host_make::{
    BaseInitializationContract, ConfigurationValue, ImplementationOffer, KindConfigurationField,
    KindConfigurationRule,
};
pub(super) fn contract() -> BaseInitializationContract {
    let mut fields = ["source_pool", "memory_pool"]
        .into_iter()
        .map(|key| KindConfigurationField {
            key: key.into(),
            default_value: ConfigurationValue::Text(String::new()),
            rule: KindConfigurationRule::DataLabel { maximum: 128 },
        })
        .collect::<Vec<_>>();
    fields.push(KindConfigurationField {
        key: "capture_access".into(),
        default_value: ConfigurationValue::Text(String::new()),
        rule: KindConfigurationRule::TextOneOf {
            values: vec!["operator-admitted".into()],
        },
    });
    for key in ["directory", "reference"] {
        fields.push(KindConfigurationField {
            key: key.into(),
            default_value: ConfigurationValue::Text(String::new()),
            rule: KindConfigurationRule::TextBytes { maximum: 1024 },
        });
    }
    for key in [
        "producer_host",
        "producer_boot",
        "producer_plan",
        "producer_play",
        "temperature_source",
        "temperature_channel",
        "temperature_name",
        "humidity_source",
        "humidity_channel",
        "humidity_name",
    ] {
        fields.push(KindConfigurationField {
            key: key.into(),
            default_value: ConfigurationValue::Text(String::new()),
            rule: KindConfigurationRule::TextBytes { maximum: 128 },
        });
    }
    BaseInitializationContract { fields }
}
pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: conduit_std_offers::ENVIRONMENT_RECORDING_HISTORY_BASE.into(),
        implementation_id: conduit_std_offers::ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION.into(),
        implementation_revision: 1,
        target_patterns: vec![
            crate::HOSTED_TARGET_ID.into(),
            "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3".into(),
        ],
        prerequisites: vec![],
        build_feature: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_generation_contract_requires_fifteen_explicit_fields_without_tail() {
        let initialization = contract();
        assert_eq!(initialization.fields.len(), 15);
        assert!(!initialization
            .fields
            .iter()
            .any(|field| field.key == "tail"));
        let mut names = initialization
            .fields
            .iter()
            .map(|field| field.key.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 15);
    }
}
