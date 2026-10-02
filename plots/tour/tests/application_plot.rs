const PLOT_SOURCE: &str = include_str!("../main.conduit");

#[test]
fn tour_is_a_checked_host_neutral_plot_over_the_application_seam() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = conduit_plot::parse_syntax_document(PLOT_SOURCE);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_plot::expand_canonical_plot(&checked, "tour", &profile).unwrap();
    assert_eq!(expanded.gears.len(), 3);
    assert_eq!(expanded.connections.len(), 2);
    assert!(!PLOT_SOURCE.contains("Host"));
    assert!(!PLOT_SOURCE.contains("implementation"));
}
