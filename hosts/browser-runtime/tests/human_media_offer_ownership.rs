use conduit_browser_runtime::human_media::{
    browser_media_acquisition_offers, BROWSER_MEDIA_ARTIFACT, BROWSER_MEDIA_PROFILE,
};

#[test]
fn browser_media_realizations_are_owned_by_the_browser_host() {
    let catalog_source =
        include_str!("../../../crates/conduit-semantic-catalog/src/browser_human_io.rs");
    assert!(!catalog_source.contains("browser/human-media@1"));
    assert!(!catalog_source.contains("conduit-browser-runtime/human-media@1"));

    for offer in browser_media_acquisition_offers() {
        assert_eq!(
            offer.implementation.execution_profile_id.as_str(),
            BROWSER_MEDIA_PROFILE
        );
        assert_eq!(
            offer.implementation.artifact_id.as_str(),
            BROWSER_MEDIA_ARTIFACT
        );
    }
}
