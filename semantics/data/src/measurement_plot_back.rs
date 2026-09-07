//! Reviewed high-level Face and canonical Back for bounded measurement plotting.

use alloc::{format, string::ToString, vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_form::{
    check_syntax_document, parse_syntax_document, CanonicalBackCatalog, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog,
};

pub const MEASUREMENT_PLOT_FORM_KIND: &str = "data/bounded-measurement-plot";
pub const MEASUREMENT_PLOT_FORM_CONTRACT_REVISION: &str = "conduit.data/bounded-measurement-plot@1";

pub fn install_measurement_plot_form_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(KindSignature {
        kind: MEASUREMENT_PLOT_FORM_KIND.to_string(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(measurement_plot_form_definition())
        .map_err(|error| error.to_string())
}

pub fn measurement_plot_form_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(MEASUREMENT_PLOT_FORM_KIND),
        kind_contract_revision: KindContractRevision::from(MEASUREMENT_PLOT_FORM_CONTRACT_REVISION),
        inputs: vec![port(
            "window",
            crate::measurement_window_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            PortDirection::Input,
        )],
        outputs: vec![port(
            "series",
            crate::measurement_plot_series_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            PortDirection::Output,
        )],
        configuration: vec![],
    }
}

pub fn install_measurement_plot_form_back(
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
    backs: &mut CanonicalBackCatalog,
) -> Result<(), alloc::string::String> {
    let source = include_str!("../../../forms/little-seismograph/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), startup)
        .map_err(|error| format!("check bounded measurement plot Back: {error:?}"))?;
    let definition = profile
        .get(&kind_id(MEASUREMENT_PLOT_FORM_KIND))
        .ok_or_else(|| "missing bounded measurement plot definition".to_string())?;
    backs
        .insert(definition, &checked, "measurement-plot")
        .map_err(|error| format!("install bounded measurement plot Back: {error:?}"))
}

fn port(name: &str, value_kind: conduit_core::KindId, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind,
        direction,
        temporal: PortTemporal::Value,
    }
}
