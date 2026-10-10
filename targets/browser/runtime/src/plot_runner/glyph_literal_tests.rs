//! Scoped glyph constants reach the installed browser Plan and kernel.
use super::*;
const SOURCE: &str =
    include_str!("../../../../../proof/browser/fixtures/scoped-pattern-glyph.conduit");

#[test]
fn glyph_aliases_execute_as_equal_checked_constants_through_browser_kernel() {
    let projection = compact_patchbay::project_with_presentation(
        SOURCE,
        1,
        false,
        crate::installed_browser::PresentationProfile::Annotation,
    )
    .unwrap();
    assert!(projection.diagnostics.is_empty());
    let (mut session, effect) =
        TourSession::prepare("browser/glyph", "boot/glyph", SOURCE, 1).unwrap();
    let TourHostEffect::Manifestation(effect) = effect else {
        panic!("planned Boolean manifestation")
    };
    assert_eq!(effect.text.as_deref(), Some("true"));
    assert_eq!(effect.presentation_kind, "presentation/bool-value");
    assert_eq!(
        projection.checked_plot_id,
        session.fragments[0].checked_plot_id.as_str()
    );
    assert_eq!(effect.plan_id, session.fragments[0].plan_id.as_str());
    let TourProgress::Receipt(receipt) = session.advance().unwrap() else {
        panic!("completed receipt")
    };
    assert_eq!(receipt.disposition, "completed");
    assert_eq!(receipt.active_play_id, effect.active_play_id);
}

#[test]
fn immutable_pattern_and_both_ipa_branches_execute_checked_values() {
    for source in [
        include_str!("../../../../../proof/browser/fixtures/scoped-pattern-local-glyph.conduit"),
        include_str!("../../../../../proof/browser/fixtures/scoped-phonetic-glyph.conduit"),
        include_str!("../../../../../proof/browser/fixtures/scoped-phonemic-glyph.conduit"),
    ] {
        let (mut session, effect) =
            TourSession::prepare("browser/glyph", "boot/glyph", source, 1).unwrap();
        let TourHostEffect::Manifestation(effect) = effect else {
            panic!("planned Boolean manifestation")
        };
        assert_eq!(effect.text.as_deref(), Some("true"));
        let TourProgress::Receipt(receipt) = session.advance().unwrap() else {
            panic!("completed receipt")
        };
        assert_eq!(receipt.disposition, "completed");
        assert_eq!(receipt.active_play_id, effect.active_play_id);
    }
}

#[test]
fn malformed_unimported_and_missing_basis_literals_refuse_before_browser_play() {
    for source in [
        SOURCE.replace("with text/pattern/notation as r\n", ""),
        SOURCE.replace("r/^[A-Z]+$/i", "r/a\\/b/"),
        SOURCE
            .replace(
                "with text/pattern/notation as r",
                "with speech/ipa/notation as r",
            )
            .replace("r/^[A-Z]+$/i", "r/t͡ʃ/")
            .replace("r⟦^[A-Z]+$⟧i", "r/t͡ʃ/"),
    ] {
        assert!(TourSession::prepare("browser/glyph", "boot/glyph", &source, 1).is_err());
        assert!(compact_patchbay::project_with_presentation(
            &source,
            1,
            false,
            crate::installed_browser::PresentationProfile::Annotation,
        )
        .is_err());
    }
}
