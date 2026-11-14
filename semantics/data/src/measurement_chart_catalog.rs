//! One portable chart sink input, independent of device count or placement.
use alloc::{string::ToString, vec};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};
pub const MEASUREMENT_CHART_FACE_KIND: &str = "face/measurement-chart";
pub const MEASUREMENT_CHART_FACE_REVISION: &str = "conduit.data/face-measurement-chart@1";
pub fn install_measurement_chart_catalog(
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup
        .insert_structured_type("MeasurementChart", crate::measurement_chart_type())
        .map_err(|e| e.to_string())?;
    startup.insert(KindSignature {
        kind: MEASUREMENT_CHART_FACE_KIND.into(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "label".into(),
                value_type: "Text".into(),
                default: None,
            },
            StartupParameterSignature {
                name: "maximum-updates".into(),
                value_type: "Count".into(),
                default: Some("1024".into()),
            },
        ],
    })?;
    profiles
        .insert_kind(measurement_chart_face_semantic_contract())
        .map_err(|e| e.to_string())
}
pub fn measurement_chart_face_semantic_contract() -> Kind {
    let mut configuration = data_label_configuration();
    configuration.push(conduit_core::KindConfigurationField {
        key: "maximum-updates".into(),
        default_value: ConfigurationValue::U64(1024),
        rule: conduit_core::KindConfigurationRule::U64Range {
            minimum: 1,
            maximum: 4096,
        },
    });
    let mut startup_parameters = data_label_startup_front();
    startup_parameters.push(FrontStartupParameter {
        name: "maximum-updates".into(),
        value_type: kind_id(COUNT_INFO_ID),
        has_default: true,
    });
    let input = PortDescriptor {
        port_id: port_id("chart"),
        value_kind: crate::measurement_chart_type()
            .profile()
            .unwrap()
            .value_kind()
            .clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let bound = crate::measurement_chart_canonical_bytes() as u32;
    Kind {
        kind_id: kind_id(MEASUREMENT_CHART_FACE_KIND),
        kind_contract_revision: MEASUREMENT_CHART_FACE_REVISION.into(),
        startup_parameters,
        shorthand: None,
        configuration,
        inputs: vec![input.clone()],
        outputs: vec![],
        semantic_laws: vec![
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
            KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Input(input.port_id),
                contract: CheckedValueContract::new(input.value_kind, bound, vec![])
                    .expect("bounded chart"),
            }]),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: bound,
        },
    }
}
