//! Real canonical resident Source through workload review, not a forged Plan.
use super::super::initial_plots;
use conduit_core::HostAdvertisement;

const PATCHBAY: &str = include_str!("../../../../../../plots/patchbay/main.conduit");

fn selection(source: &str) -> (String, HostAdvertisement) {
    let inventory = initial_plots::reviewed_inventory(source).unwrap();
    let plot = &inventory.plots[0];
    let selected = serde_json::to_string(&[initial_plots::InitialPlotSelection {
        name: plot.name.clone(),
        source_document_id: plot.source_document_id.clone(),
        checked_plot_id: plot.checked_plot_id.clone(),
    }])
    .unwrap();
    let host = initial_plots::reviewed_browser_host(
        source,
        "review/patchbay".into(),
        "boot/patchbay".into(),
    )
    .unwrap();
    (selected, host)
}

#[test]
fn canonical_patchbay_review_keeps_the_real_typed_fore_without_acquiring_authority() {
    let (selected, host) = selection(PATCHBAY);
    let review = super::review(
        PATCHBAY,
        &selected,
        &[host],
        &crate::installed_browser::local_bases(),
    )
    .unwrap();
    assert_eq!(review.review.selected_plot_count, 1);
    assert_eq!(
        review.review.required_kinds,
        [patchbay_application::PATCHBAY_INTERACTION_KIND]
    );
    assert!(!review.review.body_plan_created);
    assert!(!review.review.play_created);
    assert!(!review.review.authority_acquired);
    assert!(!review.review.resources_acquired);
}

#[test]
fn patchbay_review_refuses_stale_identity_missing_back_and_reduced_bounds() {
    let (selected, host) = selection(PATCHBAY);
    let mut stale: serde_json::Value = serde_json::from_str(&selected).unwrap();
    stale[0]["checked_plot_id"] = serde_json::json!("foreign/checked-plot");
    assert!(super::review(
        PATCHBAY,
        &stale.to_string(),
        std::slice::from_ref(&host),
        &crate::installed_browser::local_bases()
    )
    .unwrap_err()
    .contains("stale or substituted exact identity"));
    let mut missing = host.clone();
    missing
        .capabilities
        .retain(|offer| offer.kind_id.as_str() != patchbay_application::PATCHBAY_INTERACTION_KIND);
    assert!(super::review(
        PATCHBAY,
        &selected,
        &[missing],
        &crate::installed_browser::local_bases()
    )
    .is_err());
    let mut reduced = host;
    let offered = reduced
        .capabilities
        .iter_mut()
        .find(|offer| offer.kind_id.as_str() == patchbay_application::PATCHBAY_INTERACTION_KIND)
        .unwrap();
    offered.limits.max_queue_bytes = 1;
    assert!(super::review(
        PATCHBAY,
        &selected,
        &[reduced],
        &crate::installed_browser::local_bases()
    )
    .is_err());
}

#[test]
fn patchbay_name_cannot_admit_an_arbitrary_open_root_or_substituted_fore() {
    let impostor = "plot patchbay (\n >> interaction: Text\n) {\n show: face/text(\"Message\")\n interaction >> show.text\n}\n";
    let wrong_port = PATCHBAY
        .replace(
            ">> interaction: FaceInteraction",
            ">> foreign: FaceInteraction",
        )
        .replace(
            "interaction >> owner.interaction",
            "foreign >> owner.interaction",
        );
    for source in [impostor, wrong_port.as_str()] {
        let (selected, host) = selection(source);
        let error = super::review(
            source,
            &selected,
            &[host],
            &crate::installed_browser::local_bases(),
        )
        .unwrap_err();
        assert!(error.contains("reviewed resident Fore"), "{error}");
    }
    // A type-substituted sink must also refuse before any reviewed realization.
    let wrong_type = PATCHBAY.replace("interaction: FaceInteraction", "interaction: Text");
    assert!(initial_plots::reviewed_inventory(&wrong_type).is_err());
}
