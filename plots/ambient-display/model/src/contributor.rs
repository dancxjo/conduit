//! Exact portable Fore for publishing one complete ambient contribution.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};

pub const AMBIENT_HOME_CONTRIBUTOR_KIND: &str = "face/ambient-home";
pub const AMBIENT_HOME_CONTRIBUTOR_REVISION: &str = "ambient/home-contribution@1";
pub const AMBIENT_HOME_MAXIMUM_BYTES: u32 = 8192;

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
        startup_parameters: Vec::new(),
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: port_id("observation"),
            value_kind,
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
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
    startup.insert(KindSignature {
        kind: AMBIENT_HOME_CONTRIBUTOR_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(ambient_home_contributor_kind()?)
        .map_err(|why| format!("Home catalog: {why:?}"))
}
