use super::*;

#[test]
fn canonical_clock_executes_five_ticks_then_stops_in_the_browser_kernel() {
    let source = include_str!("../../../../../plots/clock/main.conduit");
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
                assert_eq!(value.face_kind, conduit_semantic_catalog::TICK_FACE_KIND);
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

#[test]
fn resident_clock_samples_civil_time_and_manifests_through_the_browser_kernel() {
    let source = include_str!("../../../../../plots/resident-ambient-clock-weather/main.conduit");
    let (mut session, mut effect) =
        TourSession::prepare("browser/civil-clock", "boot/civil-clock", source, 1).unwrap();
    let mut reply = [0; crate::browser_civil_clock::CIVIL_CLOCK_REPLY_BYTES];
    reply[..8].copy_from_slice(&1_793_523_600_876_u64.to_le_bytes());
    reply[8..10].copy_from_slice(&(-480_i16).to_le_bytes());
    reply[10..].copy_from_slice(b"2026-11-0101:00:00");
    for _ in 0..8 {
        let progress = match effect {
            TourHostEffect::Timer(_) => session.advance().unwrap(),
            TourHostEffect::CivilClockObservation(_) => {
                session.advance_with_output(&reply).unwrap()
            }
            TourHostEffect::Manifestation(value) => {
                assert_eq!(
                    value.face_kind,
                    conduit_semantic_catalog::AMBIENT_CLOCK_FACE_KIND
                );
                assert_eq!(
                    value.text.as_deref(),
                    Some("2026-11-01 · 01:00:00 · America/Los_Angeles · Weather unavailable")
                );
                assert_eq!(session.cancel().unwrap().disposition, "cancelled");
                return;
            }
            _ => panic!("unexpected clock effect"),
        };
        let TourProgress::Effect(next) = progress else {
            panic!("resident clock stopped before a Show");
        };
        effect = *next;
    }
    panic!("resident clock did not produce a Show");
}
