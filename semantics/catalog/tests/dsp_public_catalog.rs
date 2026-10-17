//! Ordinary caller Source resolves declared DSP interfaces and exact owned Backs.
use conduit_plot::*;
use conduit_semantic_catalog::{install_dsp_source_backs, install_dsp_source_catalogs};

#[test]
fn ordinary_callers_check_and_expand_public_dsp_components_without_private_aliases() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_dsp_source_catalogs(&mut startup, &mut profiles).unwrap();
    let original = profiles.clone();
    let mut backs = CanonicalBackCatalog::new();
    install_dsp_source_backs(&profiles, &mut backs).unwrap();
    assert_eq!(original, profiles);
    for (leaf, input, output, port, flow) in [
        ("gain", "GainControl", "GainOutcome", "control", "...|"),
        (
            "one-pole",
            "OnePoleFrame",
            "OnePoleOutcome",
            "frame",
            "...|",
        ),
        (
            "experimental-q14-control-frame",
            "SharedExcitationControl",
            "SharedExcitationStage",
            "value",
            "",
        ),
    ] {
        let source = format!("with audio/dsp/{input} as Input\nwith audio/dsp/{output} as Output\nplot caller (\n >> value: Input{flow}\n result: Output{flow} >>\n) {{\n component: audio/dsp/{leaf}()\n value >> component.{port}\n component.result >> result\n}}\n");
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup)
            .unwrap_or_else(|error| panic!("{leaf}: {error:?}"));
        let expanded =
            expand_canonical_plot_for_authoring_with_backs(&checked, "caller", &profiles, &backs)
                .unwrap_or_else(|error| panic!("{leaf}: {error:?}"));
        assert!(!expanded.expanded.gears.is_empty());
    }
    for private in ["MixerPair", "MixerQuad"] {
        let source = format!("plot forbidden (\n >> value: {private}\n) {{\n}}\n");
        assert!(check_syntax_document(&parse_syntax_document(&source), &startup).is_err());
    }
}
