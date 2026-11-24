//! Checked Source and finite protocol proof; no installed builder or target run.
use conduit_body_make::{
    deployment_runtime_catalog::install_creche_catalogs, womb_runtime::*, womb_runtime_catalog::*,
};

#[test]
fn womb_is_a_checked_plot_with_a_distinct_propagation_kind_and_fore() {
    let syntax =
        conduit_plot::parse_syntax_document(include_str!("../../../plots/womb/main.conduit"));
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_creche_catalogs(&mut startup, &mut profile).unwrap();
    assert!(conduit_plot::check_syntax_document(&syntax, &startup).is_err());
    install_womb_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let womb =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "womb/main", &profile).unwrap();
    let kind = womb_kind();
    assert_eq!(kind.kind_id.as_str(), WOMB_KIND);
    assert_eq!(kind.inputs[0].value_kind.as_str(), WOMB_REQUEST_KIND);
    assert_eq!(kind.outputs[0].value_kind.as_str(), WOMB_OUTCOME_KIND);
    assert_eq!(womb_host_call_requirement().target_kind, Some(kind.kind_id));
    // A real Crèche-equipped Host is not an implicit Womb builder. The common
    // transaction mechanism does not confer eligibility for another Kind.
    let host = conduit_std_host::StdHost::new_with_creche_encounter(
        conduit_std_host::StdHostConfig {
            host_id: "host/womb-contract".into(),
            boot_id: "boot/womb-contract".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        conduit_std_host::StdHostComposition::minimal(),
        conduit_core::resource_offer(
            "test/creche-slot",
            conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
            1,
        ),
    )
    .unwrap();
    assert!(conduit_planner::default_expanded_placements(
        &womb.expanded,
        std::slice::from_ref(host.advertisement())
    )
    .is_err());
}

#[test]
fn preparation_refusals_remain_classified_and_bound_to_exact_evidence() {
    for disposition in [
        WombDisposition::Prepared,
        WombDisposition::UnavailableBuilder,
        WombDisposition::UnsupportedTarget,
        WombDisposition::StaleInvitation,
        WombDisposition::Denied,
        WombDisposition::Cancelled,
        WombDisposition::Failed,
        WombDisposition::InvalidSelection,
    ] {
        let outcome = WombOutcome {
            request: WombRequest([11; 32]),
            disposition,
            evidence_digest: [29; 32],
        };
        assert_eq!(WombOutcome::decode(&outcome.encode()), Some(outcome));
        assert!(WombOutcome::decode(&outcome.encode()[..64]).is_none());
        let mut unknown = outcome.encode();
        unknown[32] = 8;
        assert!(WombOutcome::decode(&unknown).is_none());
    }
}

#[test]
fn stem_is_a_separately_checked_member_seed_plot() {
    let legacy =
        conduit_plot::parse_syntax_document(include_str!("../../../plots/womb/main.conduit"));
    let current =
        conduit_plot::parse_syntax_document(include_str!("../../../plots/stem/main.conduit"));
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();

    install_stem_catalogs(&mut startup, &mut profile).unwrap();
    let stem = conduit_plot::check_syntax_document(&current, &startup).unwrap();
    assert!(conduit_plot::check_syntax_document(&legacy, &startup).is_err());
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&stem, "stem/main", &profile).unwrap();
    assert_eq!(expanded.expanded.gears[0].kind_id.as_str(), STEM_KIND);

    install_womb_catalogs(&mut startup, &mut profile).unwrap();
    let womb = conduit_plot::check_syntax_document(&legacy, &startup).unwrap();
    assert_ne!(stem.source_document_id, womb.source_document_id);
    assert_ne!(stem.plots[0].checked_plot_id, womb.plots[0].checked_plot_id);
    let kind = stem_kind();
    assert_eq!(kind.inputs[0].value_kind.as_str(), STEM_REQUEST_KIND);
    assert_eq!(kind.outputs[0].value_kind.as_str(), STEM_OUTCOME_KIND);
    let stem_call = stem_host_call_requirement();
    assert_eq!(stem_call.target_kind, Some(kind.kind_id));
    assert_ne!(
        stem_call.contract_id,
        womb_host_call_requirement().contract_id
    );
}
