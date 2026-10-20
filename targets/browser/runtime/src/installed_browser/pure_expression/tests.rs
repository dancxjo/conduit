//! A native test executes the actual browser offer refusal, not browser math.
use super::*;
#[test]
fn qualified_value_contract_profile_is_explicitly_unavailable_to_browser_offers() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source = "plot accept (\n >> value: Frequency near 0.01Hz\n result: Frequency near 0.1Hz >>\n) = .\n";
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "accept", &profile).unwrap();
    let program = program_from_configuration(&expanded.expanded.gears[0].configuration).unwrap();
    assert!(matches!(
        program.root.operation,
        conduit_plot::PortableExpressionOperation::ValueContract { .. }
    ));
    assert_eq!(
        offer(&program, PortTemporal::Value).unwrap_err(),
        "browser/value-contract-expression-profile-unavailable@1"
    );
}
