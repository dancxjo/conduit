use super::*;

#[test]
fn canonical_clock_executes_five_ticks_then_stops_in_the_browser_kernel() {
    let source = include_str!("../../../../../forms/clock/main.conduit");
    let (mut session, mut effect) =
        TourSession::prepare("browser/clock", "boot/clock", source, 1).unwrap();
    let mut ticks = Vec::new();
    let mut timers = 0;
    let receipt = loop {
        match effect {
            TourHostEffect::Timer(timer) => {
                assert_eq!(timer.duration_millis, 1000);
                timers += 1;
            }
            TourHostEffect::Manifestation(value) => {
                assert_eq!(
                    value.presentation_kind,
                    conduit_semantic_catalog::TICK_PRESENTATION_KIND
                );
                ticks.push(value.text.unwrap());
            }
            _ => panic!("canonical clock requested an unrelated effect"),
        }
        match session.advance().unwrap() {
            TourProgress::Effect(next) => {
                effect = *next;
                if ticks.len() == 5 {
                    break session.cancel().unwrap();
                }
            }
            TourProgress::Receipt(receipt) => {
                panic!("standing clock ended before Stop: {receipt:?}");
            }
            _ => panic!("canonical clock did not continue its exact effect sequence"),
        }
    };
    assert_eq!(receipt.disposition, "cancelled");
    assert_eq!(receipt.timer_completions, 5);
    assert_eq!(receipt.manifestation_completions, 5);
    assert_eq!(timers, 5);
    assert_eq!(ticks, ["0", "1", "2", "3", "4"]);
}
