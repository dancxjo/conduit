use super::*;

fn pulse_sinks(form: &conduit_form::ExpandedCanonicalForm) -> Vec<&str> {
    form.gears
        .iter()
        .filter(|gear| {
            matches!(
                gear.kind_id.as_str(),
                conduit_semantic_catalog::PULSE_PRESENTATION_KIND
                    | conduit_semantic_catalog::PULSE_TONE_PRESENTATION_KIND
            )
        })
        .map(|gear| gear.kind_id.as_str())
        .collect()
}

#[test]
fn pulse_light_and_tone_are_explicit_and_sound_omission_removes_its_cord() {
    let source = include_str!("../../../../../forms/firefly-choir/main.conduit");
    let (startup, catalog) = crate::installed_browser::catalogs().unwrap();
    let checked =
        conduit_form::check_syntax_document(&conduit_form::parse_syntax_document(source), &startup)
            .unwrap();
    let light_only = conduit_form::expand_canonical_form_for_authoring(
        &checked,
        "pulse-manifestation",
        &catalog,
    )
    .unwrap();
    let enriched = conduit_form::expand_canonical_form_for_authoring(
        &checked,
        "pulse-light-tone-manifestation",
        &catalog,
    )
    .unwrap();

    assert_eq!(
        pulse_sinks(&enriched.expanded),
        [
            conduit_semantic_catalog::PULSE_PRESENTATION_KIND,
            conduit_semantic_catalog::PULSE_TONE_PRESENTATION_KIND,
        ]
    );
    assert_eq!(enriched.expanded.connections.len(), 2);
    assert_eq!(
        pulse_sinks(&light_only.expanded),
        [conduit_semantic_catalog::PULSE_PRESENTATION_KIND]
    );
    assert_eq!(light_only.expanded.connections.len(), 1);
    let browser =
        crate::installed_browser::advertisement("browser/light".into(), "boot/light".into());
    assert!(browser
        .capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::PULSE_PRESENTATION_KIND));
    assert!(browser.capabilities.iter().all(|offer| {
        offer.kind_id.as_str() != conduit_semantic_catalog::PULSE_TONE_PRESENTATION_KIND
    }));
}

#[test]
fn canonical_firefly_choir_manifests_four_exact_bounded_pulses() {
    let source = include_str!("../../../../../forms/firefly-choir/main.conduit");
    let (mut session, mut effect) =
        TourSession::prepare("browser/firefly", "boot/firefly", source, 1).unwrap();
    let mut pulses = Vec::new();
    let mut rhythms = Vec::new();
    loop {
        match effect {
            TourHostEffect::Timer(timer) => {
                assert_eq!(timer.duration_millis, 240)
            }
            TourHostEffect::Manifestation(value) => match value.presentation_kind.as_str() {
                conduit_semantic_catalog::PULSE_PRESENTATION_KIND => {
                    pulses.push((value.observation_sequence, value.text.unwrap()));
                }
                conduit_semantic_catalog::RHYTHM_PRESENTATION_KIND => {
                    rhythms.push(value.text.unwrap());
                }
                _ => panic!("Firefly Choir manifested an unrelated Kind"),
            },
            _ => panic!("Firefly Choir requested an unrelated effect"),
        }
        match session.advance().unwrap() {
            TourProgress::Effect(next) => effect = *next,
            TourProgress::Receipt(receipt) => {
                assert_eq!(receipt.disposition, "completed");
                assert_eq!(receipt.timer_completions, 4);
                assert_eq!(receipt.manifestation_completions, 8);
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
    assert_eq!(
        rhythms,
        [
            "rhythm 0 · next 0 ms · period 270 ms · peer 1",
            "rhythm 0 · next 64 ms · period 262 ms · peer 2",
            "rhythm 0 · next 64 ms · period 262 ms · peer 3",
            "rhythm 0 · next 64 ms · period 262 ms · peer 4",
        ]
    );
}
