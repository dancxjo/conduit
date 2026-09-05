use conduit_form::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_rhythm_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn check(source: &str) {
    let (startup, _profile) = catalogs();
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    check_syntax_document(&syntax, &startup).unwrap();
}

#[test]
fn reusable_forms_are_checked_and_host_neutral() {
    let observation = include_str!("../../../forms/pulse-observation/main.conduit");
    let synchronization = include_str!("../../../forms/phase-synchronization/main.conduit");
    check(observation);
    check(synchronization);
    for forbidden in ["browser", "dom", "websocket", "webrtc", "url", "hostid"] {
        assert!(!observation.to_ascii_lowercase().contains(forbidden));
        assert!(!synchronization.to_ascii_lowercase().contains(forbidden));
    }
}

#[test]
fn extracted_contract_has_a_non_firefly_consumer() {
    let source = include_str!("../../../forms/heartbeat-phase-follower/main.conduit");
    check(source);
    assert!(!source.to_ascii_lowercase().contains("firefly"));
}
