use conduit_form::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};

fn check(source: &str) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_quantity_mapping_catalog(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    check_syntax_document(&syntax, &startup).unwrap();
}

#[test]
fn reusable_mapping_and_outside_consumer_are_checked_and_host_neutral() {
    let mapping = include_str!("../../../forms/quantity-range-map/main.conduit");
    let light = include_str!("../../../forms/normalized-light-intensity/main.conduit");
    check(mapping);
    check(light);
    for forbidden in [
        "pointerevent",
        "touchevent",
        "canvas",
        "web audio",
        "dom",
        "pixel",
        "browser",
    ] {
        assert!(!mapping.to_ascii_lowercase().contains(forbidden));
        assert!(!light.to_ascii_lowercase().contains(forbidden));
    }
}
