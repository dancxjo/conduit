//! The public Todo proof flags select one existing finite browser scenario.
use super::TodoJourneyArgs;
use clap::Parser;

#[derive(Parser)]
struct Arguments {
    #[command(flatten)]
    journey: TodoJourneyArgs,
}
fn parse(extra: &[&str]) -> Result<TodoJourneyArgs, clap::Error> {
    let mut arguments = vec![
        "todo-proof",
        "--state-dir",
        "state",
        "--conduit-bin",
        "conduit",
        "--output",
        "capture",
    ];
    arguments.extend_from_slice(extra);
    Arguments::try_parse_from(arguments).map(|parsed| parsed.journey)
}

#[test]
fn existing_modes_keep_their_browser_scenarios() {
    for (flags, scenario) in [
        (vec![], "first-add"),
        (vec!["--count-matrix"], "count-matrix"),
        (vec!["--cross-mask-actions"], "cross-mask"),
        (
            vec!["--cross-mask-actions", "--direct-speech"],
            "cross-mask-speech",
        ),
    ] {
        assert_eq!(parse(&flags).unwrap().browser_scenario(), scenario);
    }
}

#[test]
fn look_replacement_requires_fresh_source_and_selects_the_existing_driver() {
    assert!(parse(&["--look-replacement"]).is_err());
    let parsed = parse(&["--fresh-body-source", "todo.conduit", "--look-replacement"]).unwrap();
    assert_eq!(parsed.browser_scenario(), "look-replacement");
    assert!(!parsed.cross_mask_actions && !parsed.count_matrix && !parsed.direct_speech);
}

#[test]
fn look_replacement_cannot_mix_mutation_matrix_or_speech_scenarios() {
    for other in ["--cross-mask-actions", "--count-matrix", "--direct-speech"] {
        for pair in [["--look-replacement", other], [other, "--look-replacement"]] {
            let mut flags = vec!["--fresh-body-source", "todo.conduit"];
            flags.extend(pair);
            assert!(parse(&flags).is_err(), "ambiguous scenarios: {flags:?}");
        }
    }
    assert!(parse(&["--direct-speech"]).is_err());
    assert!(parse(&["--count-matrix", "--cross-mask-actions"]).is_err());
}
