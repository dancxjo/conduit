//! Standing ambient observation fore shared by the installed Home route.
use alloc::{format, string::String, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, ConfigurationValue,
    FrontStartupParameter, FrontValueContract, FrontValueLocation, Kind, KindConfigurationField,
    KindConfigurationRule, KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection,
    PortTemporal, TEXT_INFO_ID,
};

pub const AMBIENT_HOME_OBSERVE_KIND: &str = "ambient/resident-observation";
pub const AMBIENT_HOME_OBSERVE_REVISION: &str = "ambient/resident-observation@1";
pub const AMBIENT_HOME_OBSERVATION_BYTES: u32 = 8192;

pub fn ambient_home_observation_kind() -> Result<Kind, String> {
    let types = crate::prepare_ambient_display_types()?;
    let observation = types
        .native_types
        .iter()
        .find(|ty| ty.name == "AmbientHomeObservation")
        .ok_or("missing AmbientHomeObservation")?;
    let profile = observation.value_type.profile().map_err(debug_error)?;
    let contract = CheckedValueContract::new(
        profile.value_kind().clone(),
        AMBIENT_HOME_OBSERVATION_BYTES,
        vec![],
    )
    .map_err(debug_error)?;
    let mut tick = conduit_time::time_every_outputs().remove(0);
    tick.direction = PortDirection::Input;
    let output = port_id("observation");
    let kind = Kind {
        kind_id: kind_id(AMBIENT_HOME_OBSERVE_KIND),
        kind_contract_revision: AMBIENT_HOME_OBSERVE_REVISION.into(),
        startup_parameters: vec![FrontStartupParameter {
            name: "timezone".into(),
            value_type: kind_id(TEXT_INFO_ID),
            has_default: false,
        }],
        shorthand: None,
        inputs: vec![tick],
        outputs: vec![PortDescriptor {
            port_id: output.clone(),
            value_kind: profile.value_kind().clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: false },
            abnormal_kind: None,
        }],
        configuration: vec![KindConfigurationField {
            key: "timezone".into(),
            default_value: ConfigurationValue::Text("America/Los_Angeles".into()),
            rule: KindConfigurationRule::TextBytes { maximum: 64 },
        }],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Output(output),
                contract,
            }]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: AMBIENT_HOME_OBSERVATION_BYTES,
        },
    };
    kind.validate().map_err(debug_error)?;
    Ok(kind)
}

pub fn install_ambient_home_observation_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    let kind = ambient_home_observation_kind()?;
    startup.insert(conduit_plot::KindSignature {
        kind: AMBIENT_HOME_OBSERVE_KIND.into(),
        startup_parameters: vec![conduit_plot::StartupParameterSignature {
            name: "timezone".into(),
            value_type: "Text".into(),
            default: None,
        }],
    })?;
    startup.insert_fore(AMBIENT_HOME_OBSERVE_KIND, kind.checked_front())?;
    profile.insert_kind(kind).map_err(debug_error)
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
