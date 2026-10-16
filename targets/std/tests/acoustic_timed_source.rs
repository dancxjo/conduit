//! Preparation proof only; lifecycle acceptance requires actual parent Play.
use conduit_core::CheckedValueContract;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
#[path = "common_acoustic_value/source.rs"]
mod source;
#[test]
fn authored_timed_controls_wrap_the_existing_exact_dsp_child() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(check)
        .unwrap()
        .join()
        .unwrap();
}
fn check() {
    let source = source::composed_source() + include_str!("fixtures/acoustic-timed-frame.conduit");
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
    for name in [
        "acoustic-control-input",
        "acoustic-frame-control",
        "acoustic-after-oscillator",
        "acoustic-after-resonator",
        "acoustic-retain-frame",
    ] {
        let component = expand_canonical_plot_for_authoring(&checked, name, &profiles);
        assert!(component.is_ok(), "{name}: {component:?}");
    }
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "acoustic-timed-frame", &profiles).unwrap();
    assert!(expanded.expanded.gears.len() <= 64);
    assert!(expanded.expanded.activations.is_empty());
    assert!(expanded
        .expanded
        .gears
        .iter()
        .all(|gear| gear.kind_contract_revision.as_str()
            == conduit_plot::PURE_EXPRESSION_REVISION
            || gear.kind_id.as_str() == conduit_semantic_catalog::VALUE_PAIR_KIND));
    println!(
        "timed-control-source={:?}; child-gears={}",
        expanded.expanded.source_document_id,
        expanded.expanded.gears.len()
    );
}
