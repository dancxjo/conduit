use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

#[test]
fn authored_mask_uses_the_existing_face_interaction_show_role() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_face::install_mask_plot_value_aliases(&mut startup).unwrap();
    conduit_face::install_mask_mechanism_catalog(&mut startup, &mut profiles).unwrap();
    let syntax = parse_syntax_document(include_str!("../main.conduit"));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "seven-segment", &profiles).unwrap();
    let role = conduit_face::MaskPlot::admit(&expanded).unwrap();
    assert_eq!(role.plot_name, "seven-segment");
}
