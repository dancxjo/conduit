#![cfg(feature = "semantic-bindings")]
//! Replay independently verified original pitch Source calls through the public owner.
use conduit_speech::bounded_pitch_programs::{Limits, PitchPrograms, PROGRAMS};

#[test]
#[ignore = "requires original independently verified continuous-word onset evidence"]
fn prepared_pitch_owner_preserves_all_original_onset_calls() {
    let path = std::env::var("CONDUIT_VERIFIED_PITCH_ONSETS").unwrap();
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let needed = PitchPrograms::requirements(PROGRAMS).unwrap();
    for limits in [
        Limits {
            retained: needed.retained_bound - 1,
            preparation: needed.preparation_bound,
        },
        Limits {
            retained: needed.retained_bound,
            preparation: needed.preparation_bound - 1,
        },
    ] {
        assert!(PitchPrograms::prepare(PROGRAMS, limits).is_err());
    }
    let programs = PitchPrograms::prepare(
        PROGRAMS,
        Limits {
            retained: needed.retained_bound,
            preparation: needed.preparation_bound,
        },
    )
    .unwrap();
    assert!(programs.storage().actual_retained <= programs.storage().retained_bound);
    let mut calls = 0;
    for query in original["queries"].as_array().unwrap() {
        for execution in query["source_executions"].as_array().unwrap() {
            let index = execution["program_index"].as_u64().unwrap() as usize;
            if index < 3 {
                continue;
            }
            let input: Vec<u8> = serde_json::from_value(execution["input"].clone()).unwrap();
            let expected: Vec<u8> = serde_json::from_value(execution["output"].clone()).unwrap();
            let (hex, output) = programs.execute(index - 3, &input).unwrap();
            assert_eq!(hex, original["source_programs"][index].as_str().unwrap());
            assert_eq!(output, expected);
            calls += 1;
        }
    }
    assert_eq!(calls, 360);
    assert!(programs.execute(3, &[]).is_err());
    for index in 0..3 {
        assert!(programs.execute(index, &[]).is_err());
        assert!(programs.original_program(index).is_some());
    }
    println!("PASS public prepared pitch owner: all360 original Source outputs/program identities; one-under component quotas and malformed input refused; no whole renderer/queue resource claim");
}
