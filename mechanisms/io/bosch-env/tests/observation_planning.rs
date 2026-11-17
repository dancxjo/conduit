//! Source realization and ordinary planning proof; no initialized hardware offer.
#![cfg(feature = "kernel-step")]
use conduit_bosch_env::typed::{ObservationContract, KIND, KIND_REVISION};
use conduit_core::{verify_plan, BaseImplementationId, CheckedPlotId, SourceDocumentId};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical, PlannerError};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, expand_canonical_plot_for_authoring,
    expand_canonical_plot_with_backs, parse_syntax_document, CanonicalBackCatalog,
    CanonicalBackError, ProfileCatalog, StartupCatalog,
};
#[path = "observation_planning/fixtures.rs"]
mod fixtures;
const SOURCE: &str = include_str!("../../../../plots/device-protocols/main.conduit");
#[test]
fn exact_native_front_and_selected_source_back_keep_original_checked_identity() {
    let contract = ObservationContract::prepare().unwrap();
    let kind = contract.kind().unwrap();
    kind.validate().unwrap();
    assert_eq!(kind.kind_id.as_str(), KIND);
    assert_eq!(kind.kind_contract_revision.as_str(), KIND_REVISION);
    let owner =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let plot = owner
        .plots
        .iter()
        .find(|p| p.name == "bme280-observation")
        .unwrap();
    assert_eq!(kind.checked_front(), plot.checked_front());
    let original =
        expand_canonical_plot_for_authoring(&owner, "bme280-observation", &ProfileCatalog::new())
            .unwrap();
    let mut backs = CanonicalBackCatalog::new();
    backs
        .insert_exact(
            &kind,
            &kind.startup_parameters,
            &owner,
            "bme280-observation",
            &owner.source_document_id,
            &plot.checked_plot_id,
        )
        .unwrap();
    let definition = backs.checked_definition(&kind.kind_id).unwrap();
    assert_eq!(
        definition.realization.source_document_id,
        owner.source_document_id
    );
    assert_eq!(definition.realization.checked_plot_id, plot.checked_plot_id);
    assert_eq!(
        definition.realization.kind_contract_revision,
        kind.kind_contract_revision
    );
    let mut stale = CanonicalBackCatalog::new();
    assert!(matches!(
        stale.insert_exact(
            &kind,
            &kind.startup_parameters,
            &owner,
            "bme280-observation",
            &SourceDocumentId::from("fixture/stale-source"),
            &plot.checked_plot_id
        ),
        Err(CanonicalBackError::StaleSourceDocument { .. })
    ));
    assert!(matches!(
        stale.insert_exact(
            &kind,
            &kind.startup_parameters,
            &owner,
            "bme280-observation",
            &owner.source_document_id,
            &CheckedPlotId::from("fixture/stale-checked")
        ),
        Err(CanonicalBackError::StaleCheckedPlot { .. })
    ));
    assert!(stale.checked_definition(&kind.kind_id).is_none());
    let definitions = fixtures::definitions(&kind);
    let profiles = fixtures::profiles(&definitions);
    let startup = profiles.startup_catalog().unwrap();
    let caller = check_syntax_document(&parse_syntax_document(fixtures::CALLER), &startup).unwrap();
    let direct = expand_canonical_plot(&caller, "main", &profiles).unwrap();
    assert_eq!(direct.gears.len(), 3);
    assert_eq!(
        direct
            .gears
            .iter()
            .filter(|g| g.kind_id == kind.kind_id)
            .count(),
        1
    );
    assert!(direct.realization_backs.is_empty());
    let recursive = expand_canonical_plot_with_backs(&caller, "main", &profiles, &backs).unwrap();
    assert_eq!(recursive.source_document_id, direct.source_document_id);
    assert_eq!(recursive.checked_plot_id, direct.checked_plot_id);
    assert_ne!(recursive.expanded_plot_id, direct.expanded_plot_id);
    assert_eq!(recursive.gears.len(), original.expanded.gears.len() + 2);
    assert!(recursive.gears.iter().all(|g| g.kind_id != kind.kind_id));
    assert_eq!(recursive.realization_backs.len(), 1);
    assert_eq!(
        recursive.realization_backs[0].source_document_id,
        owner.source_document_id
    );
    assert_eq!(
        recursive.realization_backs[0].checked_plot_id,
        plot.checked_plot_id
    );
    assert_eq!(recursive.source_document_id, caller.source_document_id);
}
#[test]
fn ordinary_planner_seals_native_leaf_and_refuses_missing_or_insufficient_fixture_resources() {
    let kind = ObservationContract::prepare().unwrap().kind().unwrap();
    let definitions = fixtures::definitions(&kind);
    let profiles = fixtures::profiles(&definitions);
    let checked = check_syntax_document(
        &parse_syntax_document(fixtures::CALLER),
        &profiles.startup_catalog().unwrap(),
    )
    .unwrap();
    let expanded = expand_canonical_plot(&checked, "main", &profiles).unwrap();
    let host = fixtures::host(&definitions);
    let placements = default_expanded_placements(&expanded, core::slice::from_ref(&host)).unwrap();
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    let plan =
        plan_expanded_canonical(&expanded, core::slice::from_ref(&host), &placements, &bases)
            .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.source_document_id, checked.source_document_id);
    assert_eq!(plan.checked_plot_id, expanded.checked_plot_id);
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(plan.fragments[0].placements.len(), 3);
    assert_eq!(plan.fragments[0].connections.len(), 2);
    assert!(plan.realization_backs.is_empty());
    let selected = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id == kind.kind_id)
        .unwrap();
    assert_eq!(selected.kind_contract_revision, kind.kind_contract_revision);
    assert_eq!(selected.host_id, host.host_id);
    assert_eq!(selected.boot_id, host.boot_id);
    assert!(selected.host_calls.is_empty());
    assert_eq!(selected.resources.len(), 1);
    let mut missing = host.clone();
    missing
        .capabilities
        .retain(|cap| cap.kind_id != kind.kind_id);
    assert!(default_expanded_placements(&expanded, &[missing]).is_err());
    let mut absent = host.clone();
    absent.resources.clear();
    assert!(matches!(
        plan_expanded_canonical(&expanded, &[absent], &placements, &bases),
        Err(PlannerError::UnavailableResource(_))
    ));
    let mut short = host;
    short.resources[0].capacity_units = 1;
    assert!(matches!(
        plan_expanded_canonical(&expanded, &[short], &placements, &bases),
        Err(PlannerError::ResourceCapacityExceeded(_))
    ));
}
