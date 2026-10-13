use super::*;
fn placement() -> PlannedGear {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profiles).unwrap();
    let document = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(
            "plot exact {\n value: math/realize(request={value:0.3,maximum-error:0})\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let plot = conduit_plot::expand_canonical_plot(&document, "exact", &profiles).unwrap();
    let mut host = crate::StdHost::new();
    host.prepare_numerical_realizations(&plot).unwrap();
    let hosts = [host.advertisement().clone()];
    let choices = conduit_planner::default_expanded_placements(&plot, &hosts).unwrap();
    let map = std::collections::BTreeMap::new();
    conduit_planner::plan_expanded_canonical_with_options(
        &plot,
        &hosts,
        &choices,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &map,
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8192,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
    .fragments[0]
        .placements[0]
        .clone()
}
#[test]
fn initialized_factory_rejects_relabelled_or_changed_selected_offer() {
    let correct = placement();
    let back = admitted(&correct).unwrap();
    assert!(back.retained_capacity_bytes() >= back.result_bytes().len());
    assert!(budget(&correct).unwrap().value_bytes > 0);
    let mut invalid = vec![];
    let mut changed = correct.clone();
    changed.artifact_id = "foreign/artifact".into();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.implementation_id = INTEGER.into();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.execution_profile_id = "foreign/profile".into();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.capability_id = "foreign/capability".into();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.outputs.clear();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.semantic_contract.laws.clear();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.limits.max_queue_bytes = 1;
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.realization_properties.clear();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.configuration.push(changed.configuration[0].clone());
    invalid.push(changed);
    for changed in invalid {
        assert!(admitted(&changed).is_err());
        assert!(budget(&changed).is_err());
    }
    let mut values = HostedValueStore::new(1, 8192, 8192).unwrap();
    assert!(matches!(
        prepare(&correct, &mut values).unwrap(),
        InstalledBack::Numerical(_)
    ));
}
