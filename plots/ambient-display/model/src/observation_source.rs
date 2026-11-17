//! A clock-triggered complete Home observation, including explicit absence.
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};

pub const AMBIENT_OBSERVE_KIND: &str = "ambient/home-observation";
pub const AMBIENT_OBSERVE_REVISION: &str = "ambient/home-observation@1";

pub fn ambient_observe_kind() -> Result<Kind, String> {
    let mut kind = crate::ambient_home_contributor_kind()?;
    let mut output = kind.inputs.remove(0);
    let value_contract = kind.semantic_contract().value_contracts()[0]
        .contract
        .clone();
    output.direction = PortDirection::Output;
    kind.kind_id = kind_id(AMBIENT_OBSERVE_KIND);
    kind.kind_contract_revision = AMBIENT_OBSERVE_REVISION.into();
    kind.startup_parameters = vec![
        FrontStartupParameter {
            name: "timezone".into(),
            value_type: kind_id(TEXT_INFO_ID),
            has_default: false,
        },
        FrontStartupParameter {
            name: "maximum-updates".into(),
            value_type: kind_id(COUNT_INFO_ID),
            has_default: true,
        },
    ];
    kind.inputs = vec![PortDescriptor {
        port_id: port_id("tick"),
        value_kind: kind_id("value/tick@1"),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    }];
    kind.outputs = vec![output];
    kind.configuration = vec![
        KindConfigurationField {
            key: "timezone".into(),
            default_value: ConfigurationValue::Text(String::new()),
            rule: KindConfigurationRule::TextBytes { maximum: 64 },
        },
        KindConfigurationField {
            key: "maximum-updates".into(),
            default_value: ConfigurationValue::U64(1440),
            rule: KindConfigurationRule::U64Range {
                minimum: 1,
                maximum: 4096,
            },
        },
    ];
    kind.semantic_laws = vec![
        KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Output(port_id("observation")),
            contract: value_contract,
        }]),
        KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
    ];
    kind.validate()
        .map_err(|why| format!("Home observation Kind: {why:?}"))?;
    Ok(kind)
}

/// Register the Kind after the shared ambient domain Types are installed.
pub fn install_ambient_observe(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: AMBIENT_OBSERVE_KIND.into(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "timezone".into(),
                value_type: "Text".into(),
                default: None,
            },
            StartupParameterSignature {
                name: "maximum-updates".into(),
                value_type: "Count".into(),
                default: Some("1440".into()),
            },
        ],
    })?;
    profile
        .insert_kind(ambient_observe_kind()?)
        .map_err(|why| format!("Home observation catalog: {why:?}"))?;
    crate::resident::install_resident_observe(startup, profile)
}
