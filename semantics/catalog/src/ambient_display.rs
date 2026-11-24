//! Assembly entrance to the ambient observation domain.
pub use conduit_ambient_display::{
    install_ambient_display_types, prepare_ambient_display_types, AMBIENT_DISPLAY_TYPES_SOURCE,
};

use alloc::{format, string::String, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, ConfigurationValue,
    FrontStartupParameter, FrontValueContract, FrontValueLocation, Kind, KindConfigurationField,
    KindConfigurationRule, KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection,
    PortTemporal, TEXT_INFO_ID,
};

pub const AMBIENT_CLOCK_KIND: &str = "ambient/clock-weather-observation";
pub const AMBIENT_CLOCK_REVISION: &str = "ambient/clock-weather-observation@1";
pub const AMBIENT_CLOCK_OBSERVATION_BYTES: u32 = 4096;

fn observation_contract(name: &str, maximum_bytes: u32) -> Result<CheckedValueContract, String> {
    let ambient = prepare_ambient_display_types()?;
    let observation = ambient
        .native_types
        .iter()
        .find(|ty| ty.name == name)
        .ok_or_else(|| format!("missing ambient observation type {name}"))?;
    let profile = observation
        .value_type
        .profile()
        .map_err(|error| format!("ambient observation profile: {error:?}"))?;
    CheckedValueContract::new(profile.value_kind().clone(), maximum_bytes, vec![])
        .map_err(|error| format!("ambient observation extent: {error:?}"))
}

pub fn ambient_clock_kind() -> Result<Kind, String> {
    let contract = observation_contract("AmbientClockWeather", AMBIENT_CLOCK_OBSERVATION_BYTES)?;
    let mut tick = conduit_time::time_every_outputs().remove(0);
    tick.direction = PortDirection::Input;
    let observation_port = port_id("observation");
    let kind = Kind {
        kind_id: kind_id(AMBIENT_CLOCK_KIND),
        kind_contract_revision: AMBIENT_CLOCK_REVISION.into(),
        startup_parameters: vec![FrontStartupParameter {
            name: "timezone".into(),
            value_type: kind_id(TEXT_INFO_ID),
            has_default: false,
        }],
        shorthand: None,
        inputs: vec![tick],
        outputs: vec![PortDescriptor {
            port_id: observation_port.clone(),
            value_kind: contract.value_kind.clone(),
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
                location: FrontValueLocation::Output(observation_port),
                contract,
            }]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: AMBIENT_CLOCK_OBSERVATION_BYTES,
        },
    };
    kind.validate()
        .map_err(|error| format!("ambient clock kind: {error:?}"))?;
    Ok(kind)
}

pub fn install_ambient_clock_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    let kind = ambient_clock_kind()?;
    startup.insert(conduit_plot::KindSignature {
        kind: AMBIENT_CLOCK_KIND.into(),
        startup_parameters: vec![conduit_plot::StartupParameterSignature {
            name: "timezone".into(),
            value_type: "Text".into(),
            default: None,
        }],
    })?;
    startup.insert_fore(AMBIENT_CLOCK_KIND, kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|error| format!("ambient clock catalog: {error:?}"))?;
    Ok(())
}

pub const AMBIENT_CLOCK_FACE_KIND: &str = "face/ambient-clock-weather";
pub const AMBIENT_CLOCK_FACE_REVISION: &str = "face/ambient-clock-weather@1";

pub const AMBIENT_WALLPAPER_FACE_KIND: &str = "face/ambient-wallpaper-selection";
pub const AMBIENT_WALLPAPER_FACE_REVISION: &str = "face/ambient-wallpaper-selection@1";

pub fn ambient_clock_face_kind() -> Result<Kind, String> {
    observation_face_kind(
        AMBIENT_CLOCK_FACE_KIND,
        AMBIENT_CLOCK_FACE_REVISION,
        observation_contract("AmbientClockWeather", AMBIENT_CLOCK_OBSERVATION_BYTES)?,
    )
}

pub fn ambient_wallpaper_face_kind() -> Result<Kind, String> {
    observation_face_kind(
        AMBIENT_WALLPAPER_FACE_KIND,
        AMBIENT_WALLPAPER_FACE_REVISION,
        observation_contract("AmbientWallpaperSelection", 4096)?,
    )
}

fn observation_face_kind(
    name: &str,
    revision: &str,
    contract: CheckedValueContract,
) -> Result<Kind, String> {
    let configuration = crate::data_label_configuration();
    let input = port_id("input");
    let kind = Kind {
        kind_id: kind_id(name),
        kind_contract_revision: revision.into(),
        startup_parameters: crate::startup_front(&configuration),
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: input.clone(),
            value_kind: contract.value_kind.clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: false },
            abnormal_kind: None,
        }],
        outputs: vec![],
        configuration,
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Input(input),
                contract: contract.clone(),
            }]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: contract.maximum_bytes,
        },
    };
    kind.validate()
        .map_err(|error| format!("ambient Face kind {name}: {error:?}"))?;
    Ok(kind)
}

pub fn install_ambient_clock_face_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_observation_face_kind(
        AMBIENT_CLOCK_FACE_KIND,
        ambient_clock_face_kind()?,
        startup,
        profile,
    )
}

pub fn install_ambient_wallpaper_face_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_observation_face_kind(
        AMBIENT_WALLPAPER_FACE_KIND,
        ambient_wallpaper_face_kind()?,
        startup,
        profile,
    )
}

fn install_observation_face_kind(
    name: &str,
    kind: Kind,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    startup.insert(conduit_plot::KindSignature {
        kind: name.into(),
        startup_parameters: crate::data_label_startup_signature(),
    })?;
    startup.insert_fore(name, kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|error| format!("{error:?}"))
}
