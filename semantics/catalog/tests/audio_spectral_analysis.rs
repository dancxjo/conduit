use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
#[test]
fn checked_measurement_fore_and_named_startup_are_one_ordinary_catalog_contract() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_audio_spectral_analysis_catalog(&mut startup, &mut profile)
        .unwrap();
    let source = include_str!("../../../plots/audio-spectrum-analysis/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "measure", &profile).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    assert_eq!(
        expanded.expanded.gears[0].kind_id.as_str(),
        conduit_semantic_catalog::AUDIO_SPECTRAL_ANALYSIS_KIND
    );
    let invalid = source.replace("window-frames = 32", "window-frames = 1");
    let accepted = check_syntax_document(&parse_syntax_document(&invalid), &startup)
        .ok()
        .and_then(|checked| {
            expand_canonical_plot_for_authoring(&checked, "measure", &profile).ok()
        });
    assert!(
        accepted.is_none(),
        "inadmissible short window must refuse before Play"
    );
}
