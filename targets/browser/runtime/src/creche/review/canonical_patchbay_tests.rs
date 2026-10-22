//! Birth review of the actual resident Source; no kernel or browser enactment.
use super::super::{review, InitialPlotSelection};
use conduit_core::HostAdvertisement;

const SOURCE: &str = include_str!("../../../../../../plots/patchbay/main.conduit");
fn selected(source: &str, name: &str) -> String {
    let inventory = crate::creche::initial_plots::reviewed_inventory(source).unwrap();
    let plot = inventory
        .plots
        .iter()
        .find(|plot| plot.name == name)
        .unwrap();
    serde_json::to_string(&vec![InitialPlotSelection {
        name: plot.name.clone(),
        source_document_id: plot.source_document_id.clone(),
        checked_plot_id: plot.checked_plot_id.clone(),
    }])
    .unwrap()
}
fn host() -> HostAdvertisement {
    crate::installed_browser::advertisement("review/host".into(), "review/boot".into())
}

#[test]
fn actual_canonical_patchbay_is_reviewed_without_creating_or_exercising_a_body() {
    let result = review(
        SOURCE,
        &selected(SOURCE, "patchbay"),
        &[host()],
        &crate::installed_browser::local_bases(),
    )
    .unwrap();
    assert_eq!(
        result.review.required_kinds,
        [patchbay_application::PATCHBAY_INTERACTION_KIND]
    );
    assert_eq!(result.review.reviewed_realization_count, 1);
    assert!(!result.review.body_plan_created);
    assert!(!result.review.play_created);
    assert!(!result.review.authority_acquired);
    assert!(!result.review.resources_acquired);
    assert_eq!(result.required_capabilities.len(), 1);
    assert_eq!(result.required_capabilities[0].active_instances, 1);
}

#[test]
fn copied_patchbay_kind_does_not_admit_a_different_front_port() {
    let forged = SOURCE.replace(">> interaction:", ">> forged:").replace(
        "interaction >> owner.interaction",
        "forged >> owner.interaction",
    );
    let refusal = review(
        &forged,
        &selected(&forged, "patchbay"),
        &[host()],
        &crate::installed_browser::local_bases(),
    )
    .unwrap_err();
    assert!(refusal.contains("unsupported resident Fore"), "{refusal}");
    for extra in [
        ">> unused: FaceInteraction...|",
        "unused: FaceInteraction...| >>",
    ] {
        let forged = SOURCE.replace(") {", &format!("  {extra}\n) {{"));
        // Unused declarations may already be refused by Source checking. If
        // they are checkable, Birth review must still refuse the extra Fore.
        match crate::creche::initial_plots::reviewed_inventory(&forged) {
            Ok(_) => assert!(review(
                &forged,
                &selected(&forged, "patchbay"),
                &[host()],
                &crate::installed_browser::local_bases()
            )
            .is_err()),
            Err(refusal) => assert!(!refusal.is_empty()),
        }
    }
}

#[test]
fn unrelated_open_front_is_not_admitted_by_the_patchbay_exception() {
    let source =
        "plot open_text (\n >> text: Text\n) {\n show: face/text(\"Open\")\n text >> show\n}\n";
    let refusal = review(
        source,
        &selected(source, "open_text"),
        &[host()],
        &crate::installed_browser::local_bases(),
    )
    .unwrap_err();
    assert!(refusal.contains("unsupported resident Fore"), "{refusal}");
}

#[test]
fn canonical_source_still_requires_the_exact_installed_offer() {
    let selection = selected(SOURCE, "patchbay");
    let mut missing = host();
    missing
        .capabilities
        .retain(|offer| offer.kind_id.as_str() != patchbay_application::PATCHBAY_INTERACTION_KIND);
    assert!(review(
        SOURCE,
        &selection,
        &[missing],
        &crate::installed_browser::local_bases()
    )
    .is_err());
    let mut forged = host();
    let offer = forged
        .capabilities
        .iter_mut()
        .find(|offer| offer.kind_id.as_str() == patchbay_application::PATCHBAY_INTERACTION_KIND)
        .unwrap();
    offer.host_calls[0].maximum_input_bytes -= 1;
    assert!(review(
        SOURCE,
        &selection,
        &[forged],
        &crate::installed_browser::local_bases()
    )
    .is_err());
}
