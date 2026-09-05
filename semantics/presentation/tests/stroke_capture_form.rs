use conduit_form::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};

fn check(source: &str) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_presentation::install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    check_syntax_document(&syntax, &startup).unwrap();
}

#[test]
fn capture_and_route_consumer_are_checked_and_mechanism_free() {
    let capture = include_str!("../../../forms/bounded-stroke-capture/main.conduit");
    let route = include_str!("../../../forms/route-annotation-stroke/main.conduit");
    check(capture);
    check(route);
    for forbidden in [
        "browser",
        "dom",
        "canvas",
        "websocket",
        "pixel",
        "pointerevent",
    ] {
        assert!(!capture.to_ascii_lowercase().contains(forbidden));
        assert!(!route.to_ascii_lowercase().contains(forbidden));
    }
}
