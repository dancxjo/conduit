use super::*;

#[test]
fn matrix_has_one_obligation_per_profile_and_kind() {
    let report = build_report(&CatalogHost::ALL, None).unwrap();
    assert_eq!(report.host_profile_count, 5);
    assert_eq!(
        report.matrix_entry_count,
        report.catalog_entry_count * report.host_profile_count
    );
}

#[test]
fn exact_profile_offers_drive_positive_cells() {
    let std = build_report(&[CatalogHost::Std], None).unwrap();
    let std_missing = std
        .entries
        .iter()
        .filter(|entry| matches!(entry.coverage, Coverage::MissingImplementation))
        .map(|entry| entry.kind_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        std_missing,
        vec![
            conduit_semantic_catalog::PATCHBAY_PRESENTATION_KIND,
            conduit_semantic_catalog::PATCHBAY_GEAR_FACE_KIND,
            conduit_semantic_catalog::PATCHBAY_PORT_KIND,
            conduit_semantic_catalog::PATCHBAY_CORD_KIND,
        ]
    );
    assert!(std.entries.iter().any(|entry| {
        entry.kind_id == conduit_semantic_catalog::STATE_TOGGLE_KIND
            && matches!(entry.coverage, Coverage::Direct)
    }));

    let browser = build_report(&[CatalogHost::Browser], None).unwrap();
    assert!(browser.entries.iter().any(|entry| {
        entry.kind_id == conduit_semantic_catalog::BOOL_PRESENTATION_KIND
            && matches!(entry.coverage, Coverage::Direct)
    }));

    let os = build_report(&[CatalogHost::Conduitos], None).unwrap();
    let advertised = profiles::advertisement(CatalogHost::Conduitos).unwrap();
    assert_eq!(
        os.entries
            .iter()
            .filter(|entry| matches!(entry.coverage, Coverage::Direct))
            .count(),
        advertised.capabilities.len()
    );
    let missing = os
        .entries
        .iter()
        .filter(|entry| matches!(entry.coverage, Coverage::MissingImplementation))
        .count();
    let recursive = os
        .entries
        .iter()
        .filter(|entry| matches!(entry.coverage, Coverage::Recursive))
        .count();
    assert_eq!(
        missing + advertised.capabilities.len() + recursive,
        os.catalog_entry_count
    );
    let gear_face = os
        .entries
        .iter()
        .find(|entry| entry.kind_id == conduit_semantic_catalog::PATCHBAY_GEAR_FACE_KIND)
        .unwrap();
    assert!(matches!(gear_face.coverage, Coverage::Recursive));
    assert!(gear_face.implementation.is_none());
    assert_eq!(gear_face.recursive_implementations.len(), 10);
}

#[test]
fn profile_offer_removal_cannot_leave_a_stale_positive() {
    let inventory = inventory::derive().unwrap();
    let mut profile = profiles::advertisement(CatalogHost::Std).unwrap();
    let index = profile
        .capabilities
        .iter()
        .position(|capability| {
            inventory
                .entries
                .iter()
                .any(|entry| entry.kind_id == capability.kind_id.as_str())
        })
        .unwrap();
    let removed = profile.capabilities.remove(index);
    let kind = inventory
        .entries
        .iter()
        .find(|entry| entry.kind_id == removed.kind_id.as_str())
        .unwrap();
    let entry = matrix_entry(&profile, kind, None, None, None);
    assert!(matches!(entry.coverage, Coverage::MissingImplementation));
    assert!(matches!(
        entry.classification,
        GapClassification::PortableImplementationMissing
            | GapClassification::MissingHostOperation
            | GapClassification::MissingResource
    ));
}

#[test]
fn stale_installed_revision_is_a_drift_error() {
    let inventory = inventory::derive().unwrap();
    let mut profile = profiles::advertisement(CatalogHost::Std).unwrap();
    let capability = profile
        .capabilities
        .iter_mut()
        .find(|capability| {
            inventory
                .entries
                .iter()
                .any(|entry| entry.kind_id == capability.kind_id.as_str())
        })
        .unwrap();
    capability.kind_contract_revision =
        conduit_core::KindContractRevision::from("stale/revision@0");
    let error = validate_catalog_revisions(&profile, &inventory.entries).unwrap_err();
    assert_eq!(error.code, "installed-kind-revision-mismatch");
}

#[test]
fn report_vocabulary_preserves_future_truthful_states() {
    assert_eq!(
        serde_json::to_string(&Coverage::Recursive).unwrap(),
        "\"recursive\""
    );
    assert_eq!(
        serde_json::to_string(&Coverage::Unsupported).unwrap(),
        "\"unsupported\""
    );
}

#[test]
fn patchbay_high_level_and_subject_kinds_are_recursive_on_the_constrained_profile() {
    let report = build_report(&[CatalogHost::PatchbayConstrained], None).unwrap();
    for kind in [
        conduit_semantic_catalog::PATCHBAY_PRESENTATION_KIND,
        conduit_semantic_catalog::PATCHBAY_GEAR_FACE_KIND,
        conduit_semantic_catalog::PATCHBAY_PORT_KIND,
        conduit_semantic_catalog::PATCHBAY_CORD_KIND,
    ] {
        let entry = report
            .entries
            .iter()
            .find(|entry| entry.kind_id == kind)
            .unwrap();
        assert!(matches!(entry.coverage, Coverage::Recursive));
        assert!(entry
            .realization_id
            .as_deref()
            .unwrap()
            .starts_with("canonical-back:"));
        assert!(!entry.recursive_implementations.is_empty());
    }
}

#[test]
fn terminal_graphics_manifestation_is_direct_only_where_installed() {
    let report = build_report(&CatalogHost::ALL, None).unwrap();
    let entries = report
        .entries
        .iter()
        .filter(|entry| entry.kind_id == conduit_semantic_catalog::GRAPHICS_PRESENTATION_KIND)
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), CatalogHost::ALL.len());
    let std_profile = profiles::advertisement(CatalogHost::Std)
        .unwrap()
        .profile
        .as_str()
        .to_owned();
    let std = entries
        .iter()
        .find(|entry| entry.host_profile == std_profile)
        .unwrap();
    assert!(matches!(std.coverage, Coverage::Direct));
    assert_eq!(
        std.implementation.as_ref().unwrap().implementation_id,
        conduit_std_offers::GRAPHICS_PRESENTATION_IMPLEMENTATION
    );
    let conduitos_profile = profiles::advertisement(CatalogHost::Conduitos)
        .unwrap()
        .profile
        .as_str()
        .to_owned();
    let conduitos = entries
        .iter()
        .find(|entry| entry.host_profile == conduitos_profile)
        .unwrap();
    assert!(matches!(conduitos.coverage, Coverage::Direct));
    let conduitos_implementation = conduitos::presentation_nucleus::presentation_nucleus_offers()
        .into_iter()
        .find(|offer| {
            offer.kind_id.as_str() == conduit_semantic_catalog::GRAPHICS_PRESENTATION_KIND
        })
        .unwrap()
        .implementation
        .implementation_id;
    assert_eq!(
        conduitos.implementation.as_ref().unwrap().implementation_id,
        conduitos_implementation.as_str()
    );

    let browser_profile = profiles::advertisement(CatalogHost::Browser)
        .unwrap()
        .profile
        .as_str()
        .to_owned();
    let browser = entries
        .iter()
        .find(|entry| entry.host_profile == browser_profile)
        .unwrap();
    assert!(matches!(browser.coverage, Coverage::MissingImplementation));
}
