use super::*;

#[test]
fn canonical_firefly_choir_manifests_four_exact_bounded_pulses() {
    let source = include_str!("../../../../../forms/firefly-choir/main.conduit");
    let (mut session, mut effect) =
        TourSession::prepare("browser/firefly", "boot/firefly", source, 1).unwrap();
    let mut pulses = Vec::new();
    loop {
        match effect {
            TourHostEffect::Timer(timer) => assert_eq!(timer.duration_millis, 240),
            TourHostEffect::Manifestation(value) => {
                assert_eq!(
                    value.presentation_kind,
                    conduit_semantic_catalog::PULSE_PRESENTATION_KIND
                );
                pulses.push((value.observation_sequence, value.text.unwrap()));
            }
            _ => panic!("Firefly Choir requested an unrelated effect"),
        }
        match session.advance().unwrap() {
            TourProgress::Effect(next) => effect = *next,
            TourProgress::Receipt(receipt) => {
                assert_eq!(receipt.disposition, "completed");
                assert_eq!(receipt.timer_completions, 4);
                assert_eq!(receipt.manifestation_completions, 4);
                break;
            }
            _ => panic!("Firefly Choir did not continue its bounded effect sequence"),
        }
    }
    assert_eq!(
        pulses,
        [
            (0, "pulse 0 · 240 ms".into()),
            (1, "pulse 1 · 240 ms".into()),
            (2, "pulse 2 · 240 ms".into()),
            (3, "pulse 3 · 240 ms".into()),
        ]
    );
}
