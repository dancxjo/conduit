#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
#[path = "common/playback_fixture.rs"]
#[allow(dead_code)]
mod fixture;
use conduit_speech::{pitch_trajectory::*, playback_basis::*, playback_revision::*};
macro_rules! tape {
    ($fixture:ident,$linguistic:ident,$pitch:ident,$realized:ident,$tape:ident) => {
        let $linguistic = $fixture.linguistic();
        let $pitch = prepare_utterance_pitch(
            &$fixture.source,
            &[OfferedSegmentPitch {
                event: 0,
                admission: $linguistic.accepted().pitch(),
            }],
        )
        .unwrap();
        let $realized = $fixture.realized();
        let $tape = prepare_speech_playback_tape(
            &$realized,
            &$pitch,
            &[PlaybackLinguisticBinding {
                event: 0,
                admitted: &$linguistic,
            }],
            7,
        )
        .unwrap();
    };
}

#[test]
fn correction_checks_stable_prefix_against_exact_retained_prior_body() {
    let old = fixture::fixture_with_stability("Hello Travis", "r1", None, Some(5));
    tape!(old, linguistic, pitch, realized, tape);
    let foreign = fixture::fixture_with_stability("Hallo Travis", "r1", None, Some(5));
    let next = fixture::fixture_with_stability("Hallo Trent", "r2", Some(&foreign), Some(5));
    tape!(next, other, other_pitch, other_realized, next_tape);
    assert!(conduit_language::prepare_text_revision_lineage(
        old.lexical.tape().source(),
        next.lexical.tape().source()
    )
    .is_ok());
    assert!(PreparedPlaybackChange::correction(&tape, &next_tape).is_err());
    assert!(PreparedPlaybackChange::interpretation(Some(&tape), &next_tape).is_err());
}
