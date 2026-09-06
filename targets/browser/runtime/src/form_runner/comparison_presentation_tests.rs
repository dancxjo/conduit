//! Ordinary checked timing composition with deterministic platform completions.
use super::*;

#[test]
fn timed_input_compares_through_canonical_nested_forms_and_completes() {
    let source = comparison_source();
    let projection = compact_patchbay::project_with_presentation(
        &source,
        1,
        false,
        crate::installed_browser::PresentationProfile::PatternComparison,
    )
    .unwrap();
    assert!(projection.diagnostics.is_empty());
    let (mut session, _) = TourSession::prepare_with_profile(
        "browser/timing",
        "boot/timing",
        &source,
        1,
        MorseRealization::Direct,
        crate::installed_browser::PresentationProfile::PatternComparison,
    )
    .unwrap();
    assert_eq!(
        projection.checked_form_id,
        session.fragments[0].checked_form_id.as_str()
    );
    let mut transition = 0_u64;
    let mut clock = 0;
    let times = [100_u64, 150, 200, 250, 500];
    let mut seen = None;
    for _ in 0..80 {
        let progress = session.poll_effect().unwrap();
        let progress = match progress {
            TourProgress::Cancellation {
                active_play_id,
                placement_id,
                request_sequence,
                ..
            } => session
                .acknowledge_cancellation(&active_play_id, &placement_id, request_sequence)
                .unwrap(),
            TourProgress::Waiting { .. } => {
                let effect = session
                    .pending
                    .iter()
                    .min_by_key(|effect| match effect.effect {
                        engine::BrowserHostEffect::ClockObservation => 0,
                        engine::BrowserHostEffect::ButtonTransition => 1,
                        engine::BrowserHostEffect::Manifestation(_) => 2,
                        _ => 3,
                    })
                    .unwrap();
                let output = match effect.effect {
                    engine::BrowserHostEffect::ClockObservation => {
                        let bytes = times[clock].to_le_bytes().to_vec();
                        clock += 1;
                        Some(bytes)
                    }
                    engine::BrowserHostEffect::ButtonTransition => {
                        let bytes = conduit_semantic_catalog::button_transition_value(
                            "button/primary",
                            transition.is_multiple_of(2),
                            transition,
                        )
                        .unwrap()
                        .canonical_bytes()
                        .unwrap();
                        transition += 1;
                        Some(bytes)
                    }
                    engine::BrowserHostEffect::Manifestation(_) => None,
                    _ => panic!("completed input must not require firing a deadline"),
                };
                let play = session.active_play_id.as_str().to_owned();
                let placement = session.fragments[0].placements[usize::from(effect.request.node.0)]
                    .placement_id
                    .as_str()
                    .to_owned();
                let request = effect.request.request.0;
                session
                    .complete_effect(&play, &placement, request, output.as_deref())
                    .unwrap()
            }
            progress => progress,
        };
        match progress {
            TourProgress::Effect(effect) => {
                if let TourHostEffect::Manifestation(effect) = *effect {
                    seen = effect.text;
                }
            }
            TourProgress::Receipt(receipt) => {
                assert_eq!(receipt.disposition, "completed");
                assert_eq!(receipt.manifestation_completions, 1);
                assert_eq!(transition, 5);
                assert_eq!(clock, 5);
                let text = seen.unwrap();
                assert!(
                    text.contains("matched: true") && text.contains("score_millionths: 1000000"),
                    "{text}"
                );
                return;
            }
            _ => {}
        }
    }
    panic!("finite timing composition did not complete");
}

fn comparison_source() -> String {
    let source = "form zz-timing {\n button: input/button(maximum-transitions = 5)\n attempt: time/pressed-button-attempt(maximum-presses = 3, maximum-transitions = 5, timeout-ms = 1000ms)\n derive: derive-intervals\n normalize: normalize-durations\n compare: compare-pattern(metric = \"maximum-absolute-millionths@1\", tolerance-millionths = 0)\n show: presentation/structured-info\n button.transition > attempt.transition\n attempt.events > derive.events\n derive.intervals > normalize.intervals\n normalize.normalized > compare.candidate\n normalize.normalized > compare.template\n compare.comparison > show.input\n}\n";
    // Import the exact canonical reusable declarations, without the namesake's
    // storage-dependent root. This fixture does not claim full Secret Knock support.
    let canonical = include_str!("../../../../../forms/secret-knock/main.conduit");
    let start = canonical.find("form normalize-durations (").unwrap();
    let end = canonical.len();
    format!("{}\n{source}", &canonical[start..end])
}

#[test]
fn admitted_comparison_adapter_presents_a_distinct_non_match() {
    use crate::installed_browser::{
        comparison_presentation, pattern_comparison, PresentationProfile,
    };
    use conduit_semantic_catalog::{normalized_value, PatternComparisonInput};
    let (session, _) = TourSession::prepare_with_profile(
        "comparison/host",
        "comparison/boot",
        &comparison_source(),
        2,
        MorseRealization::Direct,
        PresentationProfile::PatternComparison,
    )
    .unwrap();
    let placement = session.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.implementation_id.as_str()
                == pattern_comparison::COMPARE_PATTERN_BROWSER_IMPLEMENTATION
        })
        .unwrap();
    let mut codec = pattern_comparison::prepare_codec(placement)
        .unwrap()
        .unwrap();
    let candidate = normalized_value(&[333_333, 1_000_000])
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let template = normalized_value(&[500_000, 1_000_000])
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert!(codec
        .execute(PatternComparisonInput::Template, &template)
        .unwrap()
        .is_none());
    let output = codec
        .execute(PatternComparisonInput::Candidate, &candidate)
        .unwrap()
        .unwrap();
    let text = comparison_presentation::text(output).unwrap();
    assert!(text.contains("matched: false"), "{text}");
    assert!(text.contains("score_millionths: 833333"), "{text}");
    assert!(text.contains("tolerance_millionths: 0"), "{text}");
    assert!(text.contains("maximum-absolute-millionths@1"), "{text}");
}
