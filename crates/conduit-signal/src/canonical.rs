use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_form::{KindSignature, StartupCatalog, StartupParameterSignature};

pub fn signal_startup_catalog() -> StartupCatalog {
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
