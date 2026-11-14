//! Exact planned facts are checked against the independently rebuilt provider.
use super::*;
#[test]
fn original_contract_evidence_is_required_and_extra_properties_are_refused() {
    let document = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(
            "plot checked (\n >> value: U64 <= 8B\n result: U64 <= 8B >>\n) = (. + 1)\n",
        ),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    let plot = conduit_plot::expand_canonical_plot_for_authoring(
        &document,
        "checked",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    let program_text = match &plot.expanded.gears[0].configuration[0].value {
        ConfigurationValue::Text(text) => text,
        _ => panic!("canonical checked program"),
    };
    let program = PortableExpressionProgram::from_canonical_hex(program_text).unwrap();
    let offer = conduit_std_offers::pure_expression_local_offer(&program).unwrap();
    assert!(
        !offer.realization_properties.is_empty(),
        "exercise actual contract custody"
    );
    let mut advertisement = crate::StdHost::new().advertisement().clone();
    advertisement.capabilities.push(offer);
    let hosts = [advertisement];
    let placements = conduit_planner::default_expanded_placements(&plot.expanded, &hosts).unwrap();
    let plan =
        conduit_planner::plan_expanded_canonical(&plot.expanded, &hosts, &placements, &[]).unwrap();
    let placement = &plan.fragments[0].placements[0];
    assert!(validate(placement).is_ok());
    let mut missing = placement.clone();
    missing.realization_properties.clear();
    assert!(validate(&missing).is_err());
    let mut additional = placement.clone();
    additional
        .realization_properties
        .push(additional.realization_properties[0].clone());
    assert!(validate(&additional).is_err());
}
