//! Exhaustive checked-portable parity for the finite neighbor policy carriers.
use crate::{
    differential::program,
    frame_parity::{field_type, record},
    generated::*,
    onset_parity::variant,
};
fn decision_tag(value: SpeechNeighborDecision) -> &'static str {
    match value {
        SpeechNeighborDecision::matched => "matched",
        SpeechNeighborDecision::mismatched => "mismatched",
        SpeechNeighborDecision::observation_unresolved => "observation_unresolved",
        SpeechNeighborDecision::unsupported_matcher => "unsupported_matcher",
    }
}
#[test]
fn every_neighbor_carrier_matches_portable_evaluation() {
    let checked = program("speech_neighbor_compare");
    let matchers = [
        ("any", SpeechNeighborMatcherKind::any),
        (
            "segment_identity",
            SpeechNeighborMatcherKind::segment_identity,
        ),
        (
            "boundary_identity",
            SpeechNeighborMatcherKind::boundary_identity,
        ),
        ("unsupported", SpeechNeighborMatcherKind::unsupported),
    ];
    let presences = [
        ("absent", SpeechNeighborPresence::absent),
        ("unknown", SpeechNeighborPresence::unknown),
        ("segment", SpeechNeighborPresence::segment),
        ("boundary", SpeechNeighborPresence::boundary),
    ];
    let identities = [
        ("matched", SpeechNeighborIdentity::matched),
        ("mismatched", SpeechNeighborIdentity::mismatched),
        ("unresolved", SpeechNeighborIdentity::unresolved),
    ];
    let mut count = 0;
    for (matcher_tag, matcher) in matchers {
        for (presence_tag, presence) in presences {
            for (identity_tag, identity) in identities {
                let input = record(
                    &checked.input_type,
                    &[
                        (
                            "matcher",
                            variant(field_type(&checked.input_type, "matcher"), matcher_tag),
                        ),
                        (
                            "presence",
                            variant(field_type(&checked.input_type, "presence"), presence_tag),
                        ),
                        (
                            "identity",
                            variant(field_type(&checked.input_type, "identity"), identity_tag),
                        ),
                    ],
                )
                .canonical_bytes()
                .unwrap();
                let output = speech_neighbor_compare(SpeechNeighborComparisonInput {
                    matcher,
                    presence,
                    identity,
                })
                .unwrap();
                assert_eq!(
                    checked.evaluate(&input).unwrap(),
                    variant(&checked.output_type, decision_tag(output))
                        .canonical_bytes()
                        .unwrap()
                );
                count += 1;
            }
        }
    }
    assert_eq!(count, 48);
}
#[test]
fn every_alternative_pair_matches_portable_evaluation() {
    let checked = program("speech_neighbor_alternative");
    let decisions = [
        SpeechNeighborDecision::matched,
        SpeechNeighborDecision::mismatched,
        SpeechNeighborDecision::observation_unresolved,
        SpeechNeighborDecision::unsupported_matcher,
    ];
    for left in decisions {
        for right in decisions {
            let input = record(
                &checked.input_type,
                &[
                    (
                        "left",
                        variant(field_type(&checked.input_type, "left"), decision_tag(left)),
                    ),
                    (
                        "right",
                        variant(
                            field_type(&checked.input_type, "right"),
                            decision_tag(right),
                        ),
                    ),
                ],
            )
            .canonical_bytes()
            .unwrap();
            let output =
                speech_neighbor_alternative(SpeechNeighborAlternativePair { left, right }).unwrap();
            assert_eq!(
                checked.evaluate(&input).unwrap(),
                variant(&checked.output_type, decision_tag(output))
                    .canonical_bytes()
                    .unwrap()
            );
            // Disjunction must preserve a decisive match in either ordering.
            if matches!(left, SpeechNeighborDecision::matched)
                || matches!(right, SpeechNeighborDecision::matched)
            {
                assert!(matches!(output, SpeechNeighborDecision::matched));
            }
        }
    }
}
