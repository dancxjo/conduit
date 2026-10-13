//! Runtime admission remains available with the source catalog feature disabled.
use conduit_core::{ConfigurationEntry, ConfigurationValue, KindConfigurationRule};
use conduit_semantic_catalog::{data_label_configuration, validate_declared_data_labels};

#[test]
fn declared_label_admission_is_available_without_plot_catalog_feature() {
    let mut fields = data_label_configuration();
    fields[0].rule = KindConfigurationRule::DataLabel { maximum: 64 };
    let configuration = |text: &str| {
        [ConfigurationEntry {
            key: "label".into(),
            value: ConfigurationValue::Text(text.into()),
        }]
    };
    assert!(validate_declared_data_labels(&configuration(&"x".repeat(64)), &fields).is_ok());
    assert!(validate_declared_data_labels(&configuration(&"x".repeat(65)), &fields).is_err());
    assert!(validate_declared_data_labels(&[], &fields).is_err());
    assert!(validate_declared_data_labels(&configuration(" \t"), &fields).is_err());
}
