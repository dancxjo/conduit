use conduit_core::{ConfigurationValue, PortTemporal};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::{
    install_quantity_observation_face_kind, quantity_observation_face_kind,
    quantity_observation_face_workspace_bytes, QUANTITY_OBSERVATION_FACE_KIND,
};
#[test]
fn authored_history_workset_expands_with_exact_typed_fore_and_explicit_label() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_quantity_observation_face_kind(&mut startup, &mut profile).unwrap();
    let source = r#"# Inspect finite reported observations; the selected Host supplies implementations.
# Producer provenance arrives as data, never as authored Host or storage bindings.
plot quantity-observation-history (
    label: Text = "Quantity history"
    >> observations: ResolvedQuantityObservationSnapshot...
) {
    history: face/quantity-observation-series(label)
    observations >> history.observations
}
"#;
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "quantity-observation-history", &profile)
            .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    assert_eq!(
        expanded.expanded.gears[0].kind_id.as_str(),
        QUANTITY_OBSERVATION_FACE_KIND
    );
    assert_eq!(expanded.input_bindings.len(), 1);
    assert!(expanded.output_bindings.is_empty());
    assert!(expanded.expanded.gears[0]
        .configuration
        .iter()
        .any(|value| value.key == "label"
            && value.value == ConfigurationValue::Text("Quantity history".into())));
    let kind = quantity_observation_face_kind().unwrap();
    assert_eq!(
        kind.inputs[0].temporal,
        PortTemporal::Flow { closes: false }
    );
    assert!(kind.outputs.is_empty());
    assert!(quantity_observation_face_workspace_bytes().unwrap() > 0);
}

#[test]
fn projection_contract_is_exact_snapshot_profile_and_bound_not_single_batch() {
    let kind = quantity_observation_face_kind().unwrap();
    let snapshot = conduit_data::checked_quantity_observation_snapshot_type().unwrap();
    let profile = snapshot.value_type.profile().unwrap();
    let batch = conduit_data::checked_quantity_observation_batch_type().unwrap();
    assert_eq!(&kind.inputs[0].value_kind, profile.value_kind());
    assert_ne!(
        &kind.inputs[0].value_kind,
        batch.value_type.profile().unwrap().value_kind()
    );
    let maximum = conduit_data::resolved_quantity_observation_snapshot_byte_bound().unwrap();
    assert_eq!(kind.limits.max_queue_bytes, maximum);
    assert_eq!(kind.limits.max_queue_items, 1);
    let contracts = kind
        .semantic_laws
        .iter()
        .find_map(|law| match law {
            conduit_core::KindSemanticLaw::ValueContracts(contracts) => Some(contracts),
            _ => None,
        })
        .unwrap();
    assert_eq!(contracts.len(), 1);
    assert_eq!(
        contracts[0].location,
        conduit_core::FrontValueLocation::Input(kind.inputs[0].port_id.clone())
    );
    assert_eq!(
        contracts[0].contract,
        conduit_core::CheckedValueContract::new(profile.value_kind().clone(), maximum, vec![])
            .unwrap()
    );
}
