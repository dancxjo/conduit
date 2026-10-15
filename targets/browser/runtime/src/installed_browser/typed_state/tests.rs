//! Source-level availability must match the installed State lifecycle.
use super::*;

fn expanded(cells: &str) -> conduit_plot::ExpandedCanonicalPlot {
    let source = format!("plot lifecycle {{\n{cells}\n}}\n");
    let syntax = conduit_plot::parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let mut startup = conduit_plot::StartupCatalog::new();
    startup
        .insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "lifecycle",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap()
    .expanded
}

#[test]
fn unsupported_keep_lifetimes_refuse_before_browser_realization_selection() {
    for duration in [
        "",
        " for this step",
        " for this wake",
        " for this boot",
        " for this body",
    ] {
        let expanded = expanded(&format!("cell: keep Boolean(true){duration}"));
        let state = &expanded.gears[0];
        assert_eq!(state.kind_id.as_str(), STATE_VALUE_KIND);
        let error = offer_for_expanded(state).unwrap_err();
        assert!(error.contains("requires this Play"), "{duration}: {error}");
        assert!(super::super::catalogs::offers_for_expanded_pure_expressions(&expanded).is_err());
    }
    // An admitted Play cell with the same initializer must not accidentally
    // supply a realization for an unsupported Step cell in the same source.
    let admitted = expanded("play: keep Frequency(440Hz) for this play");
    let installed = offer_for_expanded(&admitted.gears[0]).unwrap().unwrap();
    let mixed = expanded(
        "play: keep Frequency(440Hz) for this play\nstep: keep Frequency(440Hz) for this step",
    );
    let step = mixed
        .gears
        .iter()
        .find(|gear| {
            gear.configuration.iter().any(|entry| {
                entry.key == "retained-duration"
                    && entry.value == ConfigurationValue::Text("step".into())
            })
        })
        .unwrap();
    assert_eq!(
        offer(&initial(&step.configuration).unwrap())
            .unwrap()
            .capability_id,
        installed.capability_id,
        "the initializer alone would collide with an existing Play offer"
    );
    assert!(super::super::catalogs::offers_for_expanded_pure_expressions(&mixed).is_err());
}

#[test]
fn explicit_play_and_plain_state_default_keep_their_browser_offer() {
    let expanded = expanded("cell: keep Boolean(true) for this play");
    let state = &expanded.gears[0];
    let offer = offer_for_expanded(state).unwrap().unwrap();
    assert_eq!(
        offer.implementation.implementation_id.as_str(),
        IMPLEMENTATION
    );
    let offers = super::super::catalogs::offers_for_expanded_pure_expressions(&expanded).unwrap();
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0].capability_id, offer.capability_id);
    let mut plain = state.clone();
    plain.configuration.retain(|entry| entry.key == "initial");
    assert_eq!(
        offer_for_expanded(&plain).unwrap().unwrap().capability_id,
        offer.capability_id
    );
}
