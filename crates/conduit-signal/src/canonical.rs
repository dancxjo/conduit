use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_form::{KindSignature, StartupCatalog, StartupParameterSignature};

pub fn signal_startup_catalog() -> StartupCatalog {
    let mut catalog = primary_signal_startup_catalog();
    extend_auxiliary_startup_catalog(&mut catalog);
    catalog
}

pub fn primary_signal_startup_catalog() -> StartupCatalog {
    let mut catalog = StartupCatalog::new();
    catalog
        .insert(KindSignature {
            kind: crate::PULSE_KIND.to_string(),
            startup_parameters: vec![
                StartupParameterSignature {
                    name: "count".to_string(),
                    value_type: "Count".to_string(),
                    default: Some("16".to_string()),
                },
                StartupParameterSignature {
                    name: "period-ms".to_string(),
                    value_type: "Count".to_string(),
                    default: Some("250".to_string()),
                },
                StartupParameterSignature {
                    name: "initial".to_string(),
                    value_type: "Boolean".to_string(),
                    default: Some("false".to_string()),
                },
            ],
        })
        .expect("the Signal pulse signature is unique");
    catalog
        .insert(KindSignature {
            kind: crate::SHOW_KIND.to_string(),
            startup_parameters: Vec::new(),
        })
        .expect("the Signal presentation signature is unique");
    catalog
}

fn extend_auxiliary_startup_catalog(catalog: &mut StartupCatalog) {
    catalog
        .insert(KindSignature {
            kind: crate::trigger::TRIGGER_KIND.to_string(),
            startup_parameters: vec![StartupParameterSignature {
                name: "count".to_string(),
                value_type: "Count".to_string(),
                default: Some("1".to_string()),
            }],
        })
        .expect("the Signal trigger signature is unique");
    catalog
        .insert(KindSignature {
            kind: crate::trigger::TOGGLE_KIND.to_string(),
            startup_parameters: vec![StartupParameterSignature {
                name: "initial".to_string(),
                value_type: "Boolean".to_string(),
                default: Some("false".to_string()),
            }],
        })
        .expect("the Signal toggle signature is unique");
    catalog
        .insert(KindSignature {
            kind: crate::trigger::TOGGLE_PRESENTATION_KIND.to_string(),
            startup_parameters: Vec::new(),
        })
        .expect("the Boolean presentation signature is unique");
    for kind in [
        crate::control::LEVEL_INPUT_KIND,
        crate::control::MERGE_THREE_SIGNAL_KIND,
    ] {
        catalog
            .insert(KindSignature {
                kind: kind.to_string(),
                startup_parameters: Vec::new(),
            })
            .expect("the Signal control signature is unique");
    }
}
