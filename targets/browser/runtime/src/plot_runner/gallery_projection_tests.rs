//! Projection and execution must agree even when reusable Plots follow a root.
use super::super::{MorseRealization, TourSession};
use super::{project_with_presentation, PresentationProfile};

#[test]
fn gallery_projection_uses_the_same_closed_root_as_play() {
    for (source, name, profile) in [
        (
            include_str!("../../../../../plots/firefly-choir/main.conduit"),
            "firefly-choir",
            PresentationProfile::Annotation,
        ),
        (
            include_str!("../../../../../plots/secret-knock/main.conduit"),
            "secret-knock-demo",
            PresentationProfile::PatternComparison,
        ),
    ] {
        let projection = project_with_presentation(source, 1, false, profile).unwrap();
        let (session, _) = TourSession::prepare_with_profile(
            "browser/gallery-proof",
            "boot/gallery-proof",
            source,
            1,
            MorseRealization::Direct,
            profile,
        )
        .unwrap();
        assert_eq!(projection.plot_name, name);
        assert_eq!(
            projection.source_document_id,
            session.fragments[0].source_document_id.as_str()
        );
        assert_eq!(
            projection.checked_plot_id,
            session.fragments[0].checked_plot_id.as_str()
        );
        assert!(projection.diagnostics.is_empty());
    }
}
