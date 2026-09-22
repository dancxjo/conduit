use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = include_str!("../../../forms/homeostasis/main.conduit");

#[test]
fn homeostasis_is_one_ordinary_checked_authority_free_form() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_experience_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_homeostasis_catalogs(&mut startup, &mut profile).unwrap();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "homeostasis", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        conduit_semantic_catalog::HOMEOSTASIS_REDUCE_KIND
    );
    assert_eq!(authored.expanded.gears[0].inputs.len(), 6);
    assert_eq!(authored.expanded.gears[0].outputs.len(), 1);
    for forbidden in [
        "host/",
        "provider/",
        "authority",
        "dock",
        "prompt",
        "hungry",
    ] {
        assert!(!SOURCE.contains(forbidden), "form leaked {forbidden}");
    }
}
