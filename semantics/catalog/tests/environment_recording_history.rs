//! Semantic Source and closed inspection composition; no installed worker claim.
use conduit_core::{FrontValueLocation, KindSemanticLaw, PortDirection, PortTemporal};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::{
    environment_recording_history_kind, install_environment_recording_history_kind,
    install_quantity_observation_face_kind, ENVIRONMENT_RECORDING_HISTORY_KIND,
    ENVIRONMENT_RECORDING_HISTORY_REVISION,
};

#[test]
fn source_exposes_exact_tick_and_quantity_snapshot_without_host_configuration() {
    let kind = environment_recording_history_kind().unwrap();
    assert_eq!(kind.kind_id.as_str(), ENVIRONMENT_RECORDING_HISTORY_KIND);
    assert_eq!(
        kind.kind_contract_revision.as_str(),
        ENVIRONMENT_RECORDING_HISTORY_REVISION
    );
    assert!(kind.configuration.is_empty() && kind.startup_parameters.is_empty());
    assert_eq!(kind.inputs.len(), 1);
    let mut tick = conduit_time::time_every_outputs().remove(0);
    tick.direction = PortDirection::Input;
    assert_eq!(kind.inputs[0], tick);
    assert_eq!(conduit_time::TICK_ENCODED_LEN, 8);
    assert_eq!(kind.outputs.len(), 1);
    assert_eq!(kind.outputs[0].port_id.as_str(), "observations");
    assert_eq!(
        kind.outputs[0].temporal,
        PortTemporal::Flow { closes: false }
    );
    let snapshot = conduit_data::checked_quantity_observation_snapshot_type().unwrap();
    assert_eq!(
        &kind.outputs[0].value_kind,
        snapshot.value_type.profile().unwrap().value_kind()
    );
    let maximum = conduit_data::resolved_quantity_observation_snapshot_byte_bound().unwrap();
    assert_eq!(kind.limits.max_queue_bytes, maximum);
    assert_eq!(kind.limits.max_queue_items, 1);
    let contracts = kind
        .semantic_laws
        .iter()
        .find_map(|law| match law {
            KindSemanticLaw::ValueContracts(contracts) => Some(contracts),
            _ => None,
        })
        .unwrap();
    assert_eq!(contracts.len(), 1);
    assert_eq!(
        contracts[0].location,
        FrontValueLocation::Output(kind.outputs[0].port_id.clone())
    );
    assert_eq!(contracts[0].contract.maximum_bytes, maximum);
}

#[test]
fn original_recording_history_plot_is_closed_and_uses_ordinary_quantity_face() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    install_environment_recording_history_kind(&mut startup, &mut profile).unwrap();
    install_quantity_observation_face_kind(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../plots/environment-recording-history/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "environment-recording-history", &profile)
            .unwrap();
    assert!(expanded.input_bindings.is_empty() && expanded.output_bindings.is_empty());
    assert_eq!(expanded.expanded.gears.len(), 3);
    assert_eq!(expanded.expanded.connections.len(), 2);
    for kind in [
        "time/every",
        ENVIRONMENT_RECORDING_HISTORY_KIND,
        "face/quantity-observation-series",
    ] {
        assert!(expanded
            .expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == kind));
    }
    let recording = expanded
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == ENVIRONMENT_RECORDING_HISTORY_KIND)
        .unwrap();
    assert!(recording.configuration.is_empty());
}
