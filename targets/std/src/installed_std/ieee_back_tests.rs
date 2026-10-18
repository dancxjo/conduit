use super::*;
fn plot() -> conduit_plot::ExpandedCanonicalPlot {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profiles).unwrap();
    let source = "plot value (\n >> trigger: MathematicalValue\n result: F64 >>\n) = (0x0000000000000000)\n\nplot main {\n seed: math/realize(request={value:0,maximum-error:0})\n left: value\n right: value\n compare: math/ieee-binary64-compare-quiet(predicate=equal(empty))\n seed.result >> (.source) >> left.trigger\n seed.result >> (.source) >> right.trigger\n left.result >> compare.left\n right.result >> compare.right\n}\n";
    let document =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    conduit_plot::expand_canonical_plot(&document, "main", &profiles).unwrap()
}
fn planned() -> PlannedGear {
    let plot = plot();
    let mut host = crate::StdHost::new();
    host.prepare_numerical_realizations(&plot).unwrap();
    host.prepare_expression_realizations(&plot).unwrap();
    let hosts = [host.advertisement().clone()];
    let choices = conduit_planner::default_expanded_placements(&plot, &hosts).unwrap();
    conduit_planner::plan_expanded_canonical_with_options(
        &plot,
        &hosts,
        &choices,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
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
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == IEEE_BINARY64_QUIET_KIND)
        .unwrap()
        .clone()
}
#[test]
fn initialized_ieee_factory_recomputes_identity_and_bounds_before_play() {
    let correct = planned();
    let back = admitted(&correct).unwrap();
    assert!(back.retained_capacity_bytes() > back.output_bytes());
    assert!(budget(&correct).is_ok());
    let mut changes = vec![];
    let mut changed = correct.clone();
    changed.artifact_id = "foreign/artifact".into();
    changes.push(changed);
    let mut changed = correct.clone();
    changed.execution_profile_id = ARITHMETIC_PROFILE.into();
    changes.push(changed);
    let mut changed = correct.clone();
    changed.capability_id = "foreign/capability".into();
    changes.push(changed);
    let mut changed = correct.clone();
    changed.kind_contract_revision = "foreign/revision".into();
    changes.push(changed);
    let mut changed = correct.clone();
    changed.inputs[0].value_kind = kind_id(BOOL_INFO_ID);
    changes.push(changed);
    let mut changed = correct.clone();
    changed.semantic_contract.laws.clear();
    changes.push(changed);
    let mut changed = correct.clone();
    changed.realization_properties.clear();
    changes.push(changed);
    let mut changed = correct.clone();
    let foreign_type = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    changed.realization_properties[0] = StructuredConfigurationValue::new(
        foreign_type.profile().unwrap().value_kind().clone(),
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
            vec![0],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap(),
    )
    .unwrap();
    changes.push(changed);
    let mut changed = correct.clone();
    changed.configuration.push(changed.configuration[0].clone());
    changes.push(changed);
    let mut changed = correct.clone();
    changed.limits.max_queue_bytes = 1;
    changes.push(changed);
    for changed in changes {
        assert!(admitted(&changed).is_err());
        assert!(budget(&changed).is_err());
    }
    let mut values = HostedValueStore::new(3, 8192, 16384).unwrap();
    assert!(matches!(
        prepare(&correct, &mut values).unwrap(),
        InstalledBack::IeeeBinary64(_)
    ));
}
#[test]
fn ieee_source_preparation_is_atomic_idempotent_and_clears_owned_batch() {
    let plot = plot();
    let mut host = crate::StdHost::new();
    host.prepare_numerical_realizations(&plot).unwrap();
    let accepted = host.advertisement().clone();
    host.prepare_numerical_realizations(&plot).unwrap();
    assert_eq!(host.advertisement(), &accepted);
    let mut forged = plot.clone();
    forged.gears[0].configuration.clear();
    assert!(host.prepare_numerical_realizations(&forged).is_err());
    assert_eq!(host.advertisement(), &accepted);
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profiles).unwrap();
    let doc = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document("plot main {\n}\n"),
        &startup,
    )
    .unwrap();
    let empty = conduit_plot::expand_canonical_plot(&doc, "main", &profiles).unwrap();
    host.prepare_numerical_realizations(&empty).unwrap();
    assert!(!host.advertisement().capabilities.iter().any(owns_offer));
}
