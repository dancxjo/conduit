use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_plot::{ProfileCatalog, StartupCatalog};
use conduit_std_host::{StdHost, StdHostConfig};
fn plot(value: &str) -> conduit_plot::ExpandedCanonicalPlot {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source = format!(
        "plot numbers {{\n value: math/realize(request={{value:{value},maximum-error:1}})\n}}\n"
    );
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    conduit_plot::expand_canonical_plot(&checked, "numbers", &profile).unwrap()
}
#[test]
fn preparing_offers_changes_generation_once_and_refusal_is_transactional() {
    let mut host = StdHost::new();
    let before = host.advertisement().clone();
    let rational = plot("0.3");
    host.prepare_numerical_realizations(&rational).unwrap();
    let offered = host.advertisement().clone();
    assert_eq!(offered.offer_generation.0, before.offer_generation.0 + 1);
    assert_eq!(
        offered
            .capabilities
            .iter()
            .filter(|o| o.kind_id.as_str() == conduit_semantic_catalog::NUMERICAL_REALIZE_KIND)
            .count(),
        3
    );
    host.prepare_numerical_realizations(&rational).unwrap();
    assert_eq!(host.advertisement(), &offered);
    let why = host
        .prepare_numerical_realizations(&plot("tau"))
        .unwrap_err();
    assert!(why.contains("UnsupportedDenotation"));
    assert_eq!(host.advertisement(), &offered);
    host.prepare_numerical_realizations(&plot("0.4")).unwrap();
    assert_eq!(
        host.advertisement().offer_generation.0,
        offered.offer_generation.0 + 1
    );
    assert_eq!(
        host.advertisement()
            .capabilities
            .iter()
            .filter(|o| o.kind_id.as_str() == conduit_semantic_catalog::NUMERICAL_REALIZE_KIND)
            .count(),
        3
    );
}
#[test]
fn generation_exhaustion_never_wraps_or_partially_installs() {
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("numeric-max"),
        boot_id: BootId::from("numeric-boot"),
        offer_generation: OfferGeneration(u64::MAX),
    });
    let before = host.advertisement().clone();
    assert!(host
        .prepare_numerical_realizations(&plot("0.3"))
        .unwrap_err()
        .contains("generation exhausted"));
    assert_eq!(host.advertisement(), &before);
}
