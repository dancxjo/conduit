//! Development proof of the authored DSP child; this is not complete PCM output.
use conduit_core::{CheckedValueContract, ConfigurationValue, PortTemporal};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

fn composed_source() -> String {
    let units = [
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../semantics/audio/rate_types.conduit"),
        include_str!("../../../semantics/audio/decibel_types.conduit"),
        include_str!("../../../semantics/audio/exact_control_types.conduit"),
        include_str!("../../../semantics/audio/time_analysis_types.conduit"),
        include_str!("../../../semantics/audio/spectral_analysis_types.conduit"),
        include_str!("../../../semantics/audio/dsp/periodic-types.conduit"),
        conduit_audio::PERIODIC_FLOW_SOURCE,
        conduit_audio::PERIODIC_OSCILLATOR_SOURCE,
        conduit_audio::PERIODIC_VALIDATION_SOURCE,
        include_str!("../../../semantics/audio/dsp/resonator-recurrence.conduit"),
        include_str!("../../../semantics/audio/dsp/resonator-types.conduit"),
        include_str!("../../../semantics/audio/dsp/resonator-projection.conduit"),
        include_str!("../../../semantics/audio/dsp/resonator.conduit"),
        include_str!("fixtures/acoustic-value-frame.conduit"),
    ];
    let mut imports = Vec::new();
    let mut body = Vec::new();
    for unit in units {
        for line in unit.lines() {
            if line.starts_with("with ") {
                imports.push(line);
            } else {
                body.push(line);
            }
        }
    }
    imports.extend(body);
    imports.join("\n") + "\n"
}

#[test]
fn authored_oscillator_and_resonator_expand_as_one_bounded_value_child() {
    // Whole-source checking/preparation uses a main-thread-sized finite stack;
    // this does not change the child kernel's admitted Play storage.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(check_child)
        .unwrap()
        .join()
        .unwrap();
}

fn check_child() {
    let source = composed_source();
    let owner = source
        .split("plot acoustic-frame-control (")
        .next()
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let types =
        check_syntax_document(&parse_syntax_document(&owner), &StartupCatalog::new()).unwrap();
    let stage = types
        .native_types
        .iter()
        .find(|ty| ty.name == "AcousticStage")
        .unwrap()
        .value_type
        .clone();
    let contract =
        CheckedValueContract::new(stage.profile().unwrap().value_kind().clone(), 32768, vec![])
            .unwrap();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_value_pair_kind(
        &contract,
        &stage,
        &contract,
        &stage,
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let child =
        expand_canonical_plot_for_authoring(&checked, "acoustic-value-frame", &profiles).unwrap();
    assert!(!child.expanded.gears.is_empty());
    assert!(
        child.expanded.gears.len() <= 64,
        "{} child Gears",
        child.expanded.gears.len()
    );
    assert!(child.expanded.connections.len() <= 128);
    assert!(child
        .expanded
        .connections
        .iter()
        .all(|cord| cord.temporal == PortTemporal::Value));
    let mut pair_count = 0;
    for gear in &child.expanded.gears {
        if gear.kind_id.as_str() == conduit_semantic_catalog::VALUE_PAIR_KIND {
            pair_count += 1;
        } else {
            let [entry] = gear.configuration.as_slice() else {
                panic!("one exact expression")
            };
            let ConfigurationValue::Text(encoded) = &entry.value else {
                panic!("canonical program")
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            let offer = conduit_std_offers::pure_expression_local_offer(&program).unwrap();
            assert!(gear.accepts_realization(&offer));
            assert!(offer.host_calls.is_empty());
        }
    }
    assert_eq!(pair_count, 3);
}
