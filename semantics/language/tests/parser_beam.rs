use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
#[path = "common/parser_fixture.rs"]
mod fixture;
use fixture::*;

fn boolean(ty: &conduit_core::StructuredInfoType, value: bool) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), vec![u8::from(value)]).unwrap()
}
fn hypothesis(
    f: &Fixture,
    state: &StructuredInfoValue,
    id: u64,
    score: i64,
    active: bool,
) -> StructuredInfoValue {
    let ty = f.ty("LanguageParserRawHypothesis");
    record(
        ty,
        vec![
            ("state", state.clone()),
            ("identity", number(field_type(ty, "identity"), id)),
            (
                "score",
                StructuredInfoValue::leaf(
                    field_type(ty, "score").clone(),
                    score.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
            ("active", boolean(field_type(ty, "active"), active)),
        ],
    )
}
fn beam(f: &Fixture, slots: Vec<StructuredInfoValue>) -> StructuredInfoValue {
    record(
        f.ty("LanguageParserBeam"),
        vec![
            ("candidate0", slots[0].clone()),
            ("candidate1", slots[1].clone()),
            ("candidate2", slots[2].clone()),
            ("candidate3", slots[3].clone()),
        ],
    )
}
fn rank(f: &Fixture, mut beam: StructuredInfoValue) -> StructuredInfoValue {
    for name in [
        "language-parser-rank-0-1",
        "language-parser-rank-2-3",
        "language-parser-rank-0-2",
        "language-parser-rank-1-3",
        "language-parser-rank-1-2",
    ] {
        beam = f.run(name, &beam);
    }
    beam
}
fn agreement(f: &Fixture, beam: &StructuredInfoValue, dependent: u64) -> StructuredInfoValue {
    let ty = f.ty("LanguageParserAgreementQuery");
    let input = record(
        ty,
        vec![
            ("beam", beam.clone()),
            ("dependent", number(field_type(ty, "dependent"), dependent)),
        ],
    );
    let votes = (0..4)
        .map(|i| f.run(&format!("language-parser-edge-vote-{i}"), &input))
        .collect::<Vec<_>>();
    let mut check = f.run(
        "language-parser-agreement-start",
        &record(
            f.ty("LanguageParserVotes"),
            vec![
                ("vote0", votes[0].clone()),
                ("vote1", votes[1].clone()),
                ("vote2", votes[2].clone()),
                ("vote3", votes[3].clone()),
            ],
        ),
    );
    for i in 0..4 {
        check = f.run(&format!("language-parser-agreement-check-{i}"), &check);
    }
    f.run("language-parser-agreement-finish", &check)
}
fn flag(value: &StructuredInfoValue, name: &str) -> bool {
    let StructuredInfoValueShape::Leaf(bytes) = field(value, name).shape() else {
        panic!("flag")
    };
    bytes == [1]
}
fn relation_name(state: &StructuredInfoValue) -> &str {
    tag(field(field(state, "relation0"), "base"))
}

#[test]
fn fixture_scores_preserve_ambiguous_graphs_until_survivors_agree() {
    let mut f = Fixture::new();
    let initial = f.initial(4);
    let shift = f.step(&initial, "shift", "dep", "analysis/1");
    let first = field(&shift, "state");
    // Two reviewed interpretations of an ambiguous "old man" prefix.
    let mut adjective = field(&f.step(first, "left_arc", "amod", "analysis/1"), "state").clone();
    adjective = field(
        &f.step(&adjective, "right_arc", "root", "analysis/1"),
        "state",
    )
    .clone();
    let mut nominal = field(&f.step(first, "left_arc", "nsubj", "analysis/1"), "state").clone();
    nominal = field(
        &f.step(&nominal, "right_arc", "root", "analysis/1"),
        "state",
    )
    .clone();
    assert!(f.native_ok(&adjective));
    assert!(f.native_ok(&nominal));
    let slots = vec![
        hypothesis(&f, &adjective, 1, 300, true),
        hypothesis(&f, &nominal, 2, 800, true),
        hypothesis(&f, &initial, 3, 99000, false),
        hypothesis(&f, &initial, 4, -1000, false),
    ];
    let ranked = rank(&f, beam(&f, slots));
    assert_eq!(count(field(field(&ranked, "candidate0"), "identity")), 2);
    assert!(!flag(&agreement(&f, &ranked, 0), "agrees"));
    assert!(flag(&agreement(&f, &ranked, 1), "agrees"));
    let ty = f.ty("LanguageParserPruneRequest");
    let retained = f.run(
        "language-parser-retain",
        &record(
            ty,
            vec![
                ("beam", ranked),
                (
                    "maximum_survivors",
                    number(field_type(ty, "maximum_survivors"), 1),
                ),
            ],
        ),
    );
    let agreed = agreement(&f, &retained, 0);
    assert!(flag(&agreed, "agrees"));
    assert_eq!(count(field(&agreed, "survivors")), 1);
    assert_eq!(
        tag(field(
            field(field(&agreed, "reference"), "relation"),
            "base"
        )),
        "nsubj"
    );
    assert_eq!(relation_name(&adjective), "amod");
    let features = f.run("language-parser-features", &nominal);
    assert_eq!(count(field(&features, "next")), 2);
    assert!(!flag(&features, "buffer_exhausted"));
}

#[test]
fn unprocessed_survivor_and_exact_basis_disagreement_prevent_stability() {
    let mut f = Fixture::new();
    let initial = f.initial(2);
    let root = field(
        &f.step(&initial, "right_arc", "root", "analysis/1"),
        "state",
    )
    .clone();
    let slots = vec![
        hypothesis(&f, &root, 1, 1000, true),
        hypothesis(&f, &initial, 2, 0, true),
        hypothesis(&f, &initial, 3, 0, false),
        hypothesis(&f, &initial, 4, 0, false),
    ];
    assert!(!flag(&agreement(&f, &beam(&f, slots), 0), "agrees"));
    let wrong = replace(&root, "basis", f.basis("analysis/2"));
    let slots = vec![
        hypothesis(&f, &root, 1, 1000, true),
        hypothesis(&f, &wrong, 2, 0, true),
        hypothesis(&f, &initial, 3, 0, false),
        hypothesis(&f, &initial, 4, 0, false),
    ];
    assert!(!flag(&agreement(&f, &beam(&f, slots), 0), "agrees"));
}

fn frontier(f: &Fixture, stable: u64, committed: u64) -> StructuredInfoValue {
    let ty = f.ty("LanguageParserRawFrontier");
    record(
        ty,
        vec![
            ("basis", f.basis("analysis/1")),
            ("stable", number(field_type(ty, "stable"), stable)),
            ("committed", number(field_type(ty, "committed"), committed)),
        ],
    )
}
#[test]
fn pressure_cancellation_and_stability_gate_commitment() {
    let mut f = Fixture::new();
    let initial = f.initial(2);
    let root = field(
        &f.step(&initial, "right_arc", "root", "analysis/1"),
        "state",
    )
    .clone();
    let slots = vec![
        hypothesis(&f, &root, 1, 1000, true),
        hypothesis(&f, &initial, 2, 0, false),
        hypothesis(&f, &initial, 3, 0, false),
        hypothesis(&f, &initial, 4, 0, false),
    ];
    let agreed = agreement(&f, &beam(&f, slots), 0);
    let old = frontier(&f, 0, 0);
    let ty = f.ty("LanguageParserAdvanceRequest");
    let request = |cancelled, pressure| {
        record(
            ty,
            vec![
                ("frontier", old.clone()),
                ("agreement", agreed.clone()),
                ("cancelled", boolean(field_type(ty, "cancelled"), cancelled)),
                ("pressure", boolean(field_type(ty, "pressure"), pressure)),
            ],
        )
    };
    for (cancelled, pressure, reason) in [(true, false, "cancelled"), (false, true, "pressure")] {
        let refused = f.run(
            "language-parser-advance-stable",
            &request(cancelled, pressure),
        );
        assert!(!accepted(&refused));
        assert_eq!(field(&refused, "frontier"), &old);
        assert_eq!(tag(field(&refused, "refusal")), reason);
    }
    let stable = f.run("language-parser-advance-stable", &request(false, false));
    assert!(accepted(&stable));
    let stable = field(&stable, "frontier");
    assert_eq!(count(field(stable, "stable")), 1);
    assert_eq!(count(field(stable, "committed")), 0);
    let ty = f.ty("LanguageParserCommitRequest");
    let request = |through, pressure| {
        record(
            ty,
            vec![
                ("frontier", stable.clone()),
                ("through", number(field_type(ty, "through"), through)),
                ("cancelled", boolean(field_type(ty, "cancelled"), false)),
                ("pressure", boolean(field_type(ty, "pressure"), pressure)),
            ],
        )
    };
    assert!(!accepted(
        &f.run("language-parser-commit-frontier", &request(2, false))
    ));
    assert!(!accepted(
        &f.run("language-parser-commit-frontier", &request(1, true))
    ));
    let committed = f.run("language-parser-commit-frontier", &request(1, false));
    assert!(accepted(&committed));
    assert_eq!(count(field(field(&committed, "frontier"), "committed")), 1);
}
