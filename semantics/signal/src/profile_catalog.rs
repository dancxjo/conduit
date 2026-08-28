//! Form-checking catalog for the Signal contracts.
//!
//! This is current semantic data. It is deliberately separate from the legacy
//! hosted implementation registry in `host_profile`.

use super::{
    pulse_contract_revision, pulse_kind, pulse_outputs, show_contract_revision, show_inputs,
    show_kind, MAX_SIGNAL_COUNT,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::ConfigurationValue;
use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};

pub fn signal_profile_catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: pulse_kind(),
            kind_contract_revision: pulse_contract_revision(),
            inputs: Vec::new(),
            outputs: pulse_outputs(),
            configuration: vec![
                ConfigurationField {
                    key: "count".to_string(),
                    default_value: ConfigurationValue::U64(16),
                    validation: ConfigurationRule::U64Range {
                        minimum: 0,
                        maximum: MAX_SIGNAL_COUNT,
                    },
                },
                ConfigurationField {
                    key: "period-ms".to_string(),
                    default_value: ConfigurationValue::U64(250),
                    validation: ConfigurationRule::U64Range {
                        minimum: 0,
                        maximum: u64::MAX,
                    },
                },
                ConfigurationField {
                    key: "initial".to_string(),
                    default_value: ConfigurationValue::Bool(false),
                    validation: ConfigurationRule::Any,
                },
            ],
        })
        .expect("signal profile kinds are unique");
    catalog
        .insert(KindDefinition {
            kind_id: show_kind(),
            kind_contract_revision: show_contract_revision(),
            inputs: show_inputs(),
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("signal profile kinds are unique");
    catalog
}
