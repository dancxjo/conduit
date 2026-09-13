const FORM_SOURCE: &str = include_str!("../../../../forms/tour/main.conduit");

#[test]
fn tour_is_a_checked_host_neutral_form_over_the_application_seam() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = conduit_form::parse_syntax_document(FORM_SOURCE);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_form::expand_canonical_form(&checked, "tour", &profile).unwrap();
    assert_eq!(expanded.gears.len(), 3);
    assert_eq!(expanded.connections.len(), 2);
    assert!(!FORM_SOURCE.contains("Host"));
    assert!(!FORM_SOURCE.contains("implementation"));
}
