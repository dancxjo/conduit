use super::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

fn plot() -> ExpandedCanonicalPlot {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source = "plot project (\n >> value: Quantity <= 1800B\n result: NearQuantity <= 16384B >>\n) {\n boundary: math/near(limit=0.0001m)\n value >> boundary.value\n boundary.result >> result\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    expand_canonical_plot_for_authoring(&checked, "project", &profile)
        .unwrap()
        .expanded
}

#[test]
fn exact_fragment_reuse_and_changed_facts_match_cold_factory() {
    let plot = plot();
    let gear = &plot.gears[0];
    let host = crate::StdHost::new();
    let mut cache = NumericalPreparationCache::default();
    let original = cache.near(&plot, gear, host.advertisement()).unwrap();
    let again = cache.near(&plot, gear, host.advertisement()).unwrap();
    assert!(Arc::ptr_eq(&original, &again));
    assert_eq!(
        &*original,
        crate::installed_std::near_offers(&gear.configuration).unwrap()
    );
    assert!(original
        .iter()
        .all(|offer| offer.authority_requirements.is_empty()));
    for mutation in 0..8 {
        let mut changed_plot = plot.clone();
        let mut changed_gear = gear.clone();
        let mut changed_host = host.advertisement().clone();
        match mutation {
            0 => changed_plot.source_document_id = "changed/source".into(),
            1 => changed_plot.checked_plot_id = "changed/checked".into(),
            2 => changed_plot.expanded_plot_id = "changed/expanded".into(),
            3 => changed_gear.kind_contract_revision = "changed/law-owner@2".into(),
            4 => changed_host.boot_id = "changed/boot".into(),
            5 => changed_host.host_id = "changed/host".into(),
            6 => changed_host.profile = "changed/profile".into(),
            _ => changed_host.protocol_version += 1,
        }
        let current = cache
            .near(&changed_plot, &changed_gear, &changed_host)
            .unwrap();
        assert!(
            !Arc::ptr_eq(&original, &current),
            "changed compatibility fact {mutation}"
        );
        assert_eq!(
            &*current,
            crate::installed_std::near_offers(&changed_gear.configuration).unwrap()
        );
        assert!(cache.entries.len() <= MAX_FRAGMENTS);
    }
}

#[test]
fn exact_request_bytes_fence_identity_collisions_precision_errors_and_fifo_lifetime() {
    let plot = plot();
    let gear = &plot.gears[0];
    let host = crate::StdHost::new();
    let mut cache = NumericalPreparationCache::default();
    let old = cache.near(&plot, gear, host.advertisement()).unwrap();
    let weak = Arc::downgrade(&old);
    let mut changed = gear.clone();
    changed.configuration.clear();
    let refusal = cache
        .near(&plot, &changed, host.advertisement())
        .unwrap_err();
    assert_eq!(
        refusal,
        crate::installed_std::near_offers(&changed.configuration).unwrap_err()
    );
    assert_eq!(cache.entries.len(), 1);
    assert!(Arc::ptr_eq(
        &old,
        &cache.near(&plot, gear, host.advertisement()).unwrap()
    ));
    for i in 0..5 {
        let mut changed = gear.clone();
        changed.configuration[0].value = conduit_core::ConfigurationValue::Quantity(
            conduit_core::QuantityConfigurationValue::from_value(
                conduit_core::Quantity::parse_plot_literal(&format!("0.000{i}m")).unwrap(),
            )
            .unwrap(),
        );
        // Deliberately reuse all Source/checked/expanded identities: contents win.
        let value = cache.near(&plot, &changed, host.advertisement()).unwrap();
        assert!(!Arc::ptr_eq(&old, &value));
        assert_eq!(
            &*value,
            crate::installed_std::near_offers(&changed.configuration).unwrap()
        );
        assert_eq!(value.len(), if i == 0 { 1 } else { 2 });
    }
    assert_eq!(cache.entries.len(), MAX_FRAGMENTS);
    assert!(
        weak.upgrade().is_some(),
        "borrowed owner survives FIFO eviction"
    );
    drop(cache);
    assert!(
        weak.upgrade().is_some(),
        "borrowed owner survives cache drop"
    );
    drop(old);
    assert!(
        weak.upgrade().is_none(),
        "final owner releases immutable fragment"
    );
}
