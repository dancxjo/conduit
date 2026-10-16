use conduit_core::PortTemporal;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    structured_selector_definition, CheckedCordStage, ProfileCatalog,
};
use conduit_semantic_catalog::{prepare_dsp_source_package, DspSourcePackage};
fn profiles(package: &DspSourcePackage) -> ProfileCatalog {
    let mut profiles = package.profiles.clone();
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
    assert_eq!(package.bundle.members.len(), 11);
    let profiles = profiles(&package);
    for leaf in [
        "experimental-independent-excitation",
        "experimental-q14-frame",
        "experimental-q14-control-frame",
        "experimental-q14-two-source",
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
        (
            "SharedExcitationControl",
            "dsp/experimental-q14-control-frame",
        ),
        (
            "IndependentExcitationInput",
            "dsp/experimental-independent-excitation",
        ),
        ("SharedExcitationStage", "dsp/experimental-q14-frame"),
        ("DspDualControl", "dsp/experimental-q14-two-source"),
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

#[test]
fn compositions_reuse_exact_component_profiles_within_admitted_pair_and_node_budgets() {
    let package = prepare_dsp_source_package().unwrap();
    let profiles = profiles(&package);
    let frame = expand_canonical_plot_for_authoring(
        &package.checked,
        "dsp/experimental-q14-frame",
        &profiles,
    )
    .unwrap();
    let triple = expand_canonical_plot_for_authoring(
        &package.checked,
        "dsp/experimental-independent-excitation",
        &profiles,
    )
    .unwrap();
    let mut configurations = Vec::new();
    for name in [
        "SharedNoiseResult",
        "SharedEnvelopeResult",
        "SharedGainResult",
    ] {
        let ty = &package
            .checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type;
        let matching = frame
            .expanded
            .gears
            .iter()
            .filter(|gear| {
                gear.configuration.iter().any(|c| {
                    if c.key != "program" {
                        return false;
                    }
                    let conduit_core::ConfigurationValue::Text(encoded) = &c.value else {
                        return false;
                    };
                    conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
                        .unwrap()
                        .output_type
                        == *ty
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1, "{name}");
        configurations.push(matching[0].configuration.clone());
    }
    let raw = expand_canonical_plot_for_authoring(&package.checked, "dsp/mix-two-value", &profiles)
        .unwrap();
    let mix_type = &package
        .checked
        .native_types
        .iter()
        .find(|t| t.name == "IndependentMixerResult")
        .unwrap()
        .value_type;
    let carried_mixes = triple
        .expanded
        .gears
        .iter()
        .filter(|gear| {
            gear.configuration.iter().any(|c| {
                if c.key != "program" {
                    return false;
                }
                let conduit_core::ConfigurationValue::Text(encoded) = &c.value else {
                    return false;
                };
                conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
                    .unwrap()
                    .output_type
                    == *mix_type
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(carried_mixes.len(), 2);
    assert_eq!(
        carried_mixes[0].configuration,
        carried_mixes[1].configuration
    );
    for (entry, chains, raw_mixes, carried) in [
        ("dsp/experimental-q14-two-source", 2, 1, 0),
        ("dsp/experimental-independent-excitation", 3, 0, 2),
    ] {
        let expanded =
            expand_canonical_plot_for_authoring(&package.checked, entry, &profiles).unwrap();
        for configuration in &configurations {
            assert_eq!(
                expanded
                    .expanded
                    .gears
                    .iter()
                    .filter(|g| g.configuration == *configuration)
                    .count(),
                chains
            );
        }
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|g| g.configuration == raw.expanded.gears[0].configuration)
                .count(),
            raw_mixes
        );
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|g| g.configuration == carried_mixes[0].configuration)
                .count(),
            carried
        );
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|g| g.kind_id.as_str() == "data/pair")
                .count(),
            2
        );
        assert!(
            expanded.expanded.gears.len() + 2 <= 64,
            "{entry}: {} gears plus two boundary nodes",
            expanded.expanded.gears.len()
        );
        println!(
            "{entry}: {} gears, two boundary nodes, two pair instances",
            expanded.expanded.gears.len()
        );
    }
}

#[test]
fn carried_profiles_preserve_owned_numeric_tokens_and_explicit_context() {
    let package = prepare_dsp_source_package().unwrap();
    for (source, owner, export) in [
        (
            include_str!("../../audio/dsp/seeded-noise.conduit"),
            "dsp/noise-step-candidate",
            "audio/dsp/noise-step-carry",
        ),
        (
            include_str!("../../audio/dsp/mixer.conduit"),
            "dsp/mix-two",
            "audio/dsp/mix-two-carry",
        ),
        (
            include_str!("../../audio/dsp/envelope.conduit"),
            "dsp/envelope",
            "audio/dsp/envelope-carry",
        ),
        (
            include_str!("../../audio/dsp/gain.conduit"),
            "dsp/gain",
            "audio/dsp/gain-carry",
        ),
    ] {
        let document = parse_syntax_document(source);
        let original = document
            .plots
            .iter()
            .find(|p| p.name.text == owner)
            .unwrap()
            .expression_body
            .as_ref()
            .unwrap();
        let span = original.expression.syntax.span();
        let original = &source[span.start..span.end];
        assert!(!original.contains(".control."));
        let carried = package.exports.resolve(export).unwrap();
        let body = &carried.expression_body.as_ref().unwrap().expression;
        let conduit_plot::ExpressionSyntax::Record { fields, .. } = &body.syntax else {
            panic!("explicit carry record")
        };
        assert_eq!(fields.len(), 2);
        let context = fields.iter().find(|f| f.name.text == "context").unwrap();
        let context_span = context.value.span();
        assert_eq!(
            &body.text[context_span.start - body.span.start..context_span.end - body.span.start],
            ".context"
        );
        let outcome = fields
            .iter()
            .find(|f| f.name.text == "outcome")
            .unwrap()
            .value
            .span();
        let numeric = &body.text[outcome.start - body.span.start..outcome.end - body.span.start];
        assert_eq!(
            numeric
                .replace(".control.left", ".0")
                .replace(".control.right", ".1")
                .replace(".control.", "."),
            original,
            "{owner}"
        );
    }
}
