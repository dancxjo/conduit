//! A checked unit receipt can be inspected by an exact expression in a Body Plan.
use super::initial_plots;

#[test]
fn reviewed_body_host_offers_exact_checked_pure_expressions_once() {
    let source = include_str!("../../../handbook/plots/compare-distance.conduit");
    let document = initial_plots::check_inventory(source).unwrap();
    let (startup, profile) = crate::installed_browser::catalogs_for_presentation(
        crate::installed_browser::PresentationProfile::Annotation,
    )
    .unwrap();
    let backs = crate::installed_browser::backs(&startup, &profile).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_with_backs(
        &document[0].checked,
        "compare-distance-demo",
        &profile,
        &backs,
    )
    .unwrap();
    let host = initial_plots::reviewed_browser_host(
        source,
        "browser/quantity-body".into(),
        "boot/quantity-body".into(),
    )
    .unwrap();
    let offers: Vec<_> = host
        .capabilities
        .iter()
        .filter(|offer| {
            offer.implementation.implementation_id.as_str() == "browser/kernel-pure-expression@1"
        })
        .collect();
    assert_eq!(offers.len(), 1);
    let placements = conduit_planner::default_expanded_placements(&expanded, &[host]).unwrap();
    assert_eq!(placements.by_gear.len(), expanded.gears.len());
}

#[test]
fn preparation_reuses_catalogs_without_reusing_document_receipts() {
    let first = include_str!("../../../../../proof/browser/fixtures/scoped-pattern-glyph.conduit");
    let second = first
        .replace("scoped-pattern-glyph", "scoped-pattern-second")
        .replace("[A-Z]", "[0-9]");
    let bundle = |second: &str| {
        serde_json::json!({
            "schema": "conduit.creche/reviewed-plot-bundle@2",
            "plots": [
                { "slug": "first", "entry": "scoped-pattern-glyph", "source": first },
                { "slug": "second", "entry": "scoped-pattern-second", "source": second }
            ]
        })
        .to_string()
    };
    let mut catalogs = super::catalog_preparation::CatalogPreparation::default();
    let checked =
        initial_plots::check_inventory_with_catalogs(&bundle(&second), &mut catalogs).unwrap();
    assert_eq!(checked.len(), 2);
    assert_ne!(
        checked[0].checked.source_document_id,
        checked[1].checked.source_document_id
    );
    for (entry, original) in checked.iter().zip([first, second.as_str()]) {
        let independent = initial_plots::check_source(original).unwrap();
        assert_eq!(
            entry.checked.source_document_id,
            independent.source_document_id
        );
        for (actual, expected) in entry.checked.plots.iter().zip(&independent.plots) {
            assert_eq!(actual.checked_plot_id, expected.checked_plot_id);
        }
    }
    // An earlier successful import and constructor admission must never supply
    // another document's missing lexical binding, including in a warm catalog.
    let unbound = second.replace("with text/pattern/notation as r\n", "");
    assert!(
        initial_plots::check_inventory_with_catalogs(&bundle(&unbound), &mut catalogs).is_err()
    );
    let fresh_host = initial_plots::reviewed_browser_host_with_inventory(
        &checked,
        "browser/reused-catalog".into(),
        "boot/reused-catalog".into(),
        &mut catalogs,
    )
    .unwrap();
    assert!(fresh_host.capabilities.iter().any(|offer| offer
        .implementation
        .implementation_id
        .as_str()
        == "browser/kernel-pure-expression@1"));
    assert!(initial_plots::reviewed_browser_host(
        &bundle(&unbound),
        "browser/reused-catalog".into(),
        "boot/reused-catalog".into(),
    )
    .is_err());
}

#[test]
fn preparation_retains_the_original_installed_back_receipts_across_profile_switches() {
    use crate::installed_browser::PresentationProfile::{Annotation, Quantity};
    let mut preparation = super::catalog_preparation::CatalogPreparation::default();
    for presentation in [Annotation, Quantity, Annotation] {
        let (cold_startup, cold_profile) =
            crate::installed_browser::catalogs_for_presentation(presentation).unwrap();
        let cold_backs = crate::installed_browser::backs(&cold_startup, &cold_profile).unwrap();
        let (startup, profile, backs) = preparation.get_with_backs(presentation).unwrap();
        assert_eq!(startup, &cold_startup);
        assert_eq!(profile, &cold_profile);
        // Whole equality retains the original checked Source, Plot identity,
        // startup front and realization contracts rather than just Kind names.
        assert_eq!(backs, &cold_backs);
    }
}
