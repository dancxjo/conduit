use super::*;

#[test]
fn capstone_retains_exact_cross_profile_body_and_refusal_evidence() {
    let receipt = prove("git:test-head").unwrap();
    assert_eq!(receipt.images.len(), 3);
    assert_eq!(receipt.boots.len(), 3);
    assert_eq!(receipt.part_ids.len(), 3);
    assert_eq!(receipt.initial_manifestations.manifestations.len(), 2);
    assert!(receipt
        .replaced_manifestations
        .manifestations
        .iter()
        .all(|item| item.lifecycle == ManifestationLifecycle::Replaced));
    assert_eq!(receipt.revised_manifestations.manifestations.len(), 2);
    assert_ne!(
        receipt.initial_presentation.identity,
        receipt.revised_presentation.identity
    );
    assert_eq!(
        receipt.update.source,
        "manifestation-semantic-action-to-body-truth"
    );
    assert!(receipt
        .initial_manifestations
        .manifestations
        .iter()
        .any(|item| {
            item.manifestation_id.as_str() == receipt.update.interaction_manifestation_id
        }));
    assert!(receipt.refusals.missing_live_presenter);
    assert!(receipt.refusals.headless_graphical_placement);
    assert!(receipt.refusals.stale_boot);
    assert!(receipt.refusals.stale_generation);
    assert!(receipt.refusals.cross_wired_manifestation);
    let headless = receipt
        .images
        .iter()
        .find(|item| item.profile_name == "conduitos-headless")
        .unwrap();
    assert!(headless.manifest.presenters.is_empty());
    assert!(headless.manifest.facilities.is_empty());
    for image in &receipt.images {
        assert_eq!(image.artifact_id.as_str(), image.manifest.image_id);
        assert!(!image.manifest.profile_id.is_empty());
        if image.profile_name != "conduitos-headless" {
            assert!(!image.manifest.inclusion_paths.is_empty());
        }
    }
    assert!(serde_json::to_vec(&receipt).unwrap().len() <= MAX_CAPSTONE_RECEIPT_BYTES);
}
