use conduit_audio::prepare_dsp_source_package;
use conduit_plot::{expand_canonical_plot_for_authoring, ProfileCatalog};
#[test]
fn owned_source_package_checks_all_public_component_bodies_and_exact_fronts() {
    let package = prepare_dsp_source_package().unwrap();
    assert_eq!(package.bundle.members.len(), 8);
    for leaf in [
        "oscillator-prepare",
        "oscillator-step",
        "resonator-prepare",
        "resonator-step",
        "noise-step-candidate",
        "envelope",
        "gain",
        "mix-two",
        "mix-four",
        "one-pole",
    ] {
        let export = package
            .exports
            .resolve(&format!("audio/dsp/{leaf}"))
            .unwrap();
        let expanded = expand_canonical_plot_for_authoring(
            &package.checked,
            &export.name.text,
            &ProfileCatalog::new(),
        )
        .unwrap();
        assert!(!expanded.expanded.gears.is_empty(), "{leaf}");
    }
}
