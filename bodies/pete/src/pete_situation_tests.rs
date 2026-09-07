use super::*;
use conduit_core::TemporalScale;
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
};

fn instant(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/pete-situation".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn observed<T>(value: T, sign: &str, ticks: u64) -> SituationInput<T> {
    SituationInput::Observed(Observed {
        value,
        sign_id: SignId::from(sign),
        observed_at: instant(ticks),
    })
}

#[test]
fn three_typed_families_form_one_finite_situation_with_exact_provenance() {
    let situation = select_pete_situation(
        instant(1_100),
        500,
        observed(
            BatteryObservation::new(750, 14_000).unwrap(),
            "sign/create/battery/7",
            1_000,
        ),
        observed("Pete, stop".into(), "sign/human/utterance/4", 1_050),
        observed(
            PeteBodyState {
                motion_available: true,
                safety_inhibited: false,
            },
            "sign/body/safety/9",
            1_075,
        ),
    )
    .unwrap();
    assert_eq!(situation.battery.state, SituationFactState::Current);
    assert_eq!(situation.utterance.state, SituationFactState::Current);
    assert_eq!(situation.body.state, SituationFactState::Current);
    assert_eq!(situation.battery.value.unwrap().charge_permille, 750);
    assert_eq!(
        situation.utterance.value.as_ref().unwrap().text,
        "Pete, stop"
    );
    assert_eq!(situation.body.provenance[0].sign_id, "sign/body/safety/9");
    assert!(serde_json::to_vec(&situation).unwrap().len() <= MAXIMUM_PETE_SITUATION_BYTES);
}

#[test]
fn missing_stale_unavailable_and_contradictory_remain_distinct() {
    let situation = select_pete_situation(
        instant(2_000),
        100,
        observed(
            BatteryObservation::new(500, 13_000).unwrap(),
            "sign/create/battery/old",
            1_000,
        ),
        SituationInput::Missing,
        SituationInput::Unavailable {
            source_identity: "capability/body-state".into(),
        },
    )
    .unwrap();
    assert_eq!(situation.battery.state, SituationFactState::Stale);
    assert_eq!(situation.utterance.state, SituationFactState::Missing);
    assert_eq!(situation.body.state, SituationFactState::Unavailable);

    let contradiction = select_pete_situation(
        instant(2_000),
        100,
        SituationInput::Contradictory {
            first: Observed {
                value: BatteryObservation::new(500, 13_000).unwrap(),
                sign_id: SignId::from("sign/battery/a"),
                observed_at: instant(1_990),
            },
            second: Observed {
                value: BatteryObservation::new(700, 14_000).unwrap(),
                sign_id: SignId::from("sign/battery/b"),
                observed_at: instant(1_995),
            },
        },
        SituationInput::Missing,
        SituationInput::Missing,
    )
    .unwrap();
    assert_eq!(
        contradiction.battery.state,
        SituationFactState::Contradictory
    );
    assert_eq!(contradiction.battery.provenance.len(), 2);
    assert!(contradiction.battery.value.is_none());
    assert_eq!(contradiction.battery.alternatives.len(), 2);
}

#[test]
fn temporal_uncertainty_cannot_be_promoted_to_current() {
    let mut uncertain = instant(1_950);
    uncertain.uncertainty_ticks = 100;
    let situation = select_pete_situation(
        instant(2_000),
        100,
        SituationInput::Observed(Observed {
            value: BatteryObservation::new(500, 13_000).unwrap(),
            sign_id: SignId::from("sign/battery/uncertain"),
            observed_at: uncertain,
        }),
        SituationInput::Missing,
        SituationInput::Missing,
    )
    .unwrap();
    assert_eq!(situation.battery.state, SituationFactState::Indeterminate);
}

#[test]
fn canonical_pete_situation_is_one_checked_open_form() {
    let source = include_str!("../../../forms/pete-situation/main.conduit");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_pete_situation_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "pete-situation", &profile).unwrap();
    assert_eq!(authored.input_bindings.len(), 3);
    assert_eq!(authored.output_bindings.len(), 1);
    assert_eq!(authored.expanded.gears.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        PETE_SITUATION_SELECT_KIND
    );
    for forbidden in ["Create OI", "JSON", "device", "host", "prompt", "camera"] {
        assert!(!source.contains(forbidden));
    }
}
