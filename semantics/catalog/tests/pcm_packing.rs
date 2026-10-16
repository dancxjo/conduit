use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

#[test]
fn packing_catalog_retains_native_constructor_laws_and_finite_fore() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_pcm16_packing_catalog(&mut startup, &mut profiles).unwrap();
    let kind = conduit_semantic_catalog::pcm16_packing_semantic_contract();
    kind.validate().unwrap();
    assert_eq!(kind.inputs.len(), 1);
    assert_eq!(kind.outputs.len(), 1);
    assert_eq!(kind.checked_front().value_contracts().len(), 2);
    let checked = check_syntax_document(&parse_syntax_document(r#"
plot frame (
 >> sample: I128
 result: AudioPcm16PackingInput >>
) = (AudioPcm16PackingInput.block({clock_id: 1, discontinuity: false, sample_rate_hz: 8000, samples: [.], start_frame: 0}))
"#), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "frame", &profiles).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
}
