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
