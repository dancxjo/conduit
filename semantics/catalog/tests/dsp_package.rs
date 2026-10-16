use conduit_core::PortTemporal;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    structured_selector_definition, CheckedCordStage, ProfileCatalog,
};
use conduit_semantic_catalog::{prepare_dsp_source_package, DspSourcePackage};
fn profiles(package: &DspSourcePackage) -> ProfileCatalog {
    let mut profiles = ProfileCatalog::new();
    let mut seen = std::collections::BTreeSet::new();
    for plot in &package.checked.plots {
        for cord in &plot.cords {
            for stage in &cord.stages {
                if let CheckedCordStage::StructuredSelector { selector, .. } = stage {
                    // The owned example's public streams all close explicitly.
                    let definition = structured_selector_definition(
                        selector,
                        PortTemporal::Flow { closes: true },
                    );
                    if seen.insert(definition.kind_id.clone()) {
                        profiles.insert(definition).unwrap();
                    }
                }
            }
        }
    }
    profiles
}
#[test]
fn owned_source_package_checks_all_public_component_bodies_and_exact_fronts() {
    let package = prepare_dsp_source_package().unwrap();
    assert_eq!(package.bundle.members.len(), 9);
    let profiles = profiles(&package);
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
        "noise-gain-example",
    ] {
        let export = package
            .exports
            .resolve(&format!("audio/dsp/{leaf}"))
            .unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&package.checked, &export.name.text, &profiles)
                .unwrap();
        assert!(!expanded.expanded.gears.is_empty(), "{leaf}");
    }
    assert!(package.exports.resolve("audio/dsp/not-shipped").is_none());
}
#[test]
fn shipped_control_imports_retain_the_owners_exact_front_types() {
    let package = prepare_dsp_source_package().unwrap();
    let mut startup = package.startup.clone();
    package.exports.install_shipped_types(&mut startup).unwrap();
    for (ty, owner) in [
        ("GainControl", "dsp/gain"),
        ("NoiseStepCandidate", "dsp/noise-step-candidate"),
        ("AudioPeriodicStepInput", "dsp/oscillator-step"),
        ("ResonatorStepInput", "dsp/resonator-step"),
        ("OnePoleFrame", "dsp/one-pole"),
    ] {
        let source = format!(
            "with audio/dsp/{ty} as Imported\nplot downstream (\n >> value: Imported\n) {{\n}}\n"
        );
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let owner = package
            .checked
            .plots
            .iter()
            .find(|p| p.name == owner)
            .unwrap();
        assert_eq!(
            checked.plots[0].checked_front().inputs()[0].value_kind,
            owner.checked_front().inputs()[0].value_kind,
            "{ty}"
        );
    }
}
#[test]
fn authored_imports_expand_the_same_owned_noise_and_gain_programs() {
    let package = prepare_dsp_source_package().unwrap();
    let profiles = profiles(&package);
    let example =
        expand_canonical_plot_for_authoring(&package.checked, "dsp/noise-gain-example", &profiles)
            .unwrap();
    assert!(example
        .expanded
        .gears
        .iter()
        .any(|g| g.kind_contract_revision.as_str() == "structured-info/selector-operation@1"));
    let example_configurations = example
        .expanded
        .gears
        .iter()
        .map(|g| &g.configuration)
        .collect::<Vec<_>>();
    for owner in ["dsp/noise-step-candidate", "dsp/gain"] {
        let component =
            expand_canonical_plot_for_authoring(&package.checked, owner, &profiles).unwrap();
        assert!(
            component
                .expanded
                .gears
                .iter()
                .all(|g| example_configurations.contains(&&g.configuration)),
            "missing unchanged owner {owner}"
        );
    }
}
#[test]
fn declared_value_profiles_preserve_the_exact_owned_flow_programs() {
    let package = prepare_dsp_source_package().unwrap();
    for (owner, value) in [
        ("dsp/noise-step-candidate", "dsp/noise-step-value"),
        ("dsp/envelope", "dsp/envelope-value"),
        ("dsp/gain", "dsp/gain-value"),
        ("dsp/mix-two", "dsp/mix-two-value"),
    ] {
        let owner =
            expand_canonical_plot_for_authoring(&package.checked, owner, &ProfileCatalog::new())
                .unwrap();
        let value =
            expand_canonical_plot_for_authoring(&package.checked, value, &ProfileCatalog::new())
                .unwrap();
        assert_eq!(owner.expanded.gears.len(), 1);
        assert_eq!(value.expanded.gears.len(), 1);
        assert_eq!(
            owner.expanded.gears[0].configuration,
            value.expanded.gears[0].configuration
        );
        assert_eq!(
            owner.expanded.gears[0].inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(
            value.expanded.gears[0].inputs[0].temporal,
            PortTemporal::Value
        );
    }
}
