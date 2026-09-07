use super::*;

#[test]
fn canonical_clock_executes_four_exact_tick_manifestations_in_the_browser_kernel() {
    let source = include_str!("../../../../../forms/clock/main.conduit");
    let (mut session, mut effect) =
        TourSession::prepare("browser/clock", "boot/clock", source, 1).unwrap();
    let mut ticks = Vec::new();
    let mut timers = 0;
    loop {
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
            TourProgress::Effect(next) => effect = *next,
            TourProgress::Receipt(receipt) => {
                assert_eq!(receipt.disposition, "completed");
                assert_eq!(receipt.timer_completions, 4);
                assert_eq!(receipt.manifestation_completions, 4);
                break;
            }
            _ => panic!("canonical clock did not continue its exact effect sequence"),
        }
    }
    assert_eq!(timers, 4);
    assert_eq!(ticks, ["0", "1", "2", "3"]);
}
