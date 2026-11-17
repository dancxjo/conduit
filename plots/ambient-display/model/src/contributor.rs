//! Exact portable Fore for publishing one complete ambient contribution.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, ConfigurationValue,
    FrontStartupParameter, FrontValueContract, FrontValueLocation, Kind, KindConfigurationField,
    KindConfigurationRule, KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};

pub const AMBIENT_HOME_CONTRIBUTOR_KIND: &str = "face/ambient-home";
pub const AMBIENT_HOME_CONTRIBUTOR_REVISION: &str = "ambient/home-contribution@2";
pub const AMBIENT_HOME_MAXIMUM_BYTES: u32 = 8192;
/// One day of minute observations; an admitted run may select a different
/// finite bound. Exhaustion never authorizes a Wake or automatic Play restart.
pub const AMBIENT_HOME_DEFAULT_UPDATES: u64 = 24 * 60;

pub fn ambient_home_contributor_kind() -> Result<Kind, String> {
    let checked = crate::prepare_ambient_display_types()?;
    let ty = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "AmbientHomeObservation")
        .ok_or("ambient Home Type is missing")?;
    let value_kind = ty
        .value_type
        .profile()
        .map_err(|why| format!("Home profile: {why:?}"))?
        .value_kind()
        .clone();
    let contract =
        CheckedValueContract::new(value_kind.clone(), AMBIENT_HOME_MAXIMUM_BYTES, Vec::new())
            .map_err(|why| format!("Home value bound: {why:?}"))?;
    let kind = Kind {
        kind_id: kind_id(AMBIENT_HOME_CONTRIBUTOR_KIND),
        kind_contract_revision: AMBIENT_HOME_CONTRIBUTOR_REVISION.into(),
        startup_parameters: vec![
            FrontStartupParameter {
                name: "label".into(),
                value_type: kind_id(conduit_core::TEXT_INFO_ID),
                has_default: false,
            },
            FrontStartupParameter {
                name: "maximum-updates".into(),
                value_type: kind_id(conduit_core::COUNT_INFO_ID),
                has_default: true,
            },
        ],
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: port_id("observation"),
            value_kind,
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: {
            let mut fields = conduit_core::data_label_configuration();
            fields.push(KindConfigurationField {
                key: "maximum-updates".into(),
                default_value: ConfigurationValue::U64(AMBIENT_HOME_DEFAULT_UPDATES),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: u64::MAX,
                },
            });
            fields
        },
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Input(port_id("observation")),
                contract,
            }]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: AMBIENT_HOME_MAXIMUM_BYTES,
        },
    };
    kind.validate()
        .map_err(|why| format!("Home Kind: {why:?}"))?;
    Ok(kind)
}

pub fn install_ambient_home_contributor(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    crate::install_ambient_display_types(startup)?;
    for aspect in [
        crate::AmbientDisplayAspect::Home,
        crate::AmbientDisplayAspect::Wallpaper,
        crate::AmbientDisplayAspect::ClockWeather,
    ] {
        startup.insert(KindSignature {
            kind: aspect.kind().into(),
            startup_parameters: vec![
                StartupParameterSignature {
                    name: "label".into(),
                    value_type: "Text".into(),
                    default: None,
                },
                StartupParameterSignature {
                    name: "maximum-updates".into(),
                    value_type: "Count".into(),
                    default: Some(format!("{AMBIENT_HOME_DEFAULT_UPDATES}")),
                },
            ],
        })?;
        profile
            .insert_kind(crate::ambient_aspect_contributor_kind(aspect)?)
            .map_err(|why| format!("Home catalog: {why:?}"))?;
    }
    crate::resident::install_resident_contributors(startup, profile)
}
