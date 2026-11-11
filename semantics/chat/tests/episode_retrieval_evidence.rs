#![cfg(feature = "plot-catalog")]
use conduit_chat::{
    EpisodeEvidenceOrigin, EpisodeRetrievalEvidenceError, PreparedEpisodeRetrievalEvidence,
};
use conduit_core::{TemporalRelationError, TemporalScale};
#[allow(dead_code)]
mod allocation_probe {
    use std::thread_local;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../architecture/plot/tests/common/allocation_probe.rs"
    ));
}
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

const ACTUAL: &[u8] = br#"{"revision":7,"provenance_revision":"experience/7","evidence":[{"identity":"body/battery","origin":"observation","temporal_role":"recent","utf8_content":"{\"voltage\":13,\"unit\":\"V\",\"uncertainty\":0.1,\"calibration\":\"meter/2\"}","sources":[{"sign":"battery/4"}],"observed_at":{"clock_basis":"clock/script","scale":"milliseconds","ticks":1000,"resolution_ticks":2,"uncertainty_ticks":3}},{"identity":"model/answer","origin":"model_derived","temporal_role":"current","utf8_content":"blue","sources":[{"implementation_run":{"implementation":"model/1","provider_instance":"provider/local","artifact":"artifact/1","run":"run/7"}}],"observed_at":null}]}"#;

#[test]
fn actual_compact_evidence_keeps_original_content_sources_and_exact_time() {
    let mut parser = PreparedEpisodeRetrievalEvidence::new();
    let evidence = parser.parse(ACTUAL).unwrap();
    assert_eq!(evidence.original_json, ACTUAL);
    assert_eq!(evidence.original_json.as_ptr(), ACTUAL.as_ptr());
    assert_eq!(evidence.revision, Some(7));
    assert_eq!(
        evidence.provenance_revision.unwrap().as_str(),
        "experience/7"
    );
    assert_eq!(evidence.items().len(), 2);
    let observed = &evidence.items()[0];
    assert_eq!(observed.identity.unwrap().as_str(), "body/battery");
    assert_eq!(observed.origin, Some(EpisodeEvidenceOrigin::Observation));
    let time = observed.observed_at.unwrap();
    assert_eq!(time.clock_basis.as_str(), "clock/script");
    assert_eq!(time.scale, TemporalScale::Milliseconds);
    assert_eq!(
        (time.ticks, time.resolution_ticks, time.uncertainty_ticks),
        (1000, 2, 3)
    );
    assert_eq!(time.borrowed().interval().unwrap(), (997, 1003));
    assert_eq!(
        evidence.items()[1].origin,
        Some(EpisodeEvidenceOrigin::ModelDerived)
    );
    assert_eq!(evidence.items()[1].observed_at, None);
}
#[test]
fn unknown_and_missing_metadata_stays_unknown_without_changing_original() {
    let input = br#"{"unrelated":{"numeric":1e20,"flags":[true,false,null]},"evidence":[{}, {"origin":"future_origin","observed_at":null,"future":{"a":1}}]}"#;
    let result = PreparedEpisodeRetrievalEvidence::new()
        .parse(input)
        .unwrap();
    assert_eq!(result.original_json, input);
    assert_eq!(result.revision, None);
    assert_eq!(result.provenance_revision, None);
    assert_eq!(result.items()[0].identity, None);
    assert_eq!(result.items()[0].origin, None);
    assert_eq!(result.items()[1].origin, None);
    assert_eq!(result.items()[1].observed_at, None);
}
#[test]
fn escaped_unicode_is_copied_into_fixed_metadata_buffers() {
    let input = br#"{"provenance_revision":"r/\u03bb","evidence":[{"identity":"human/\ud83d\ude42","observed_at":{"clock_basis":"clock/\u65e5","scale":"nanoseconds","ticks":9,"resolution_ticks":1,"uncertainty_ticks":0}}]}"#;
    let result = PreparedEpisodeRetrievalEvidence::new()
        .parse(input)
        .unwrap();
    assert_eq!(result.provenance_revision.unwrap().as_str(), "r/λ");
    assert_eq!(result.items()[0].identity.unwrap().as_str(), "human/🙂");
    assert_eq!(
        result.items()[0].observed_at.unwrap().clock_basis.as_str(),
        "clock/日"
    );
}
#[test]
fn surrogate_adaptation_distinguishes_escaped_backslashes_and_preserves_original() {
    for (input, expected) in [
        (
            r#"{"evidence":[{"identity":"\\ud83d\\ude42"}]}"#,
            "\\ud83d\\ude42",
        ),
        (r#"{"evidence":[{"identity":"\\\uD83D\uDe42"}]}"#, "\\🙂"),
        (
            r#"{"evidence":[{"identity":"\\\\ud83d\\\\ude42"}]}"#,
            "\\\\ud83d\\\\ude42",
        ),
        (
            r#"{"evidence":[{"identit\u0079":"\ud83d\ude42"}],"extra\uD83d\uDE42":{"key":"\u03bb\ud83d\ude42"}}"#,
            "🙂",
        ),
    ] {
        let parsed = PreparedEpisodeRetrievalEvidence::new()
            .parse(input.as_bytes())
            .unwrap();
        assert_eq!(parsed.original_json, input.as_bytes());
        assert_eq!(parsed.items()[0].identity.unwrap().as_str(), expected);
    }
    for input in [
        r#"{"evidence":[{"identity":"\ud83d"}]}"#,
        r#"{"evidence":[{"identity":"\ude42"}]}"#,
        r#"{"evidence":[{"identity":"\ud83d\ud83d"}]}"#,
        r#"{"evidence":[{"identity":"\ud83d\\ude42"}]}"#,
    ] {
        assert_eq!(
            PreparedEpisodeRetrievalEvidence::new().parse(input.as_bytes()),
            Err(EpisodeRetrievalEvidenceError::Malformed)
        );
    }
}
#[test]
fn duplicate_interpreted_fields_and_malformed_scalars_refuse() {
    for input in [
        r#"{"revision":1,"revision":2}"#,
        r#"{"evidence":[],"evidence":[]}"#,
        r#"{"evidence":[{"origin":null,"origin":"observation"}]}"#,
        r#"{"evidence":[{"identity":"x","identity":"y"}]}"#,
        r#"{"evidence":[{"observed_at":null,"observed_at":null}]}"#,
        r#"{"evidence":[{"sources":[],"sources":[]}]}"#,
        r#"{"evidence":[{"utf8_content":"a","utf8_content":"b"}]}"#,
        r#"{"revision":-1}"#,
        r#"{"revision":18446744073709551616}"#,
        r#"{"revision":1.5}"#,
        r#"{"unknown":truegarbage}"#,
        r#"{"unknown":01}"#,
        r#"{"unknown":NaN}"#,
        r#"{"evidence":[{"identity":3}]}"#,
    ] {
        assert_eq!(
            PreparedEpisodeRetrievalEvidence::new().parse(input.as_bytes()),
            Err(EpisodeRetrievalEvidenceError::Malformed),
            "{input}"
        );
    }
}
fn instant(fields: &str) -> String {
    format!(r#"{{"evidence":[{{"observed_at":{{{fields}}}}}]}}"#)
}
#[test]
fn temporal_metadata_requires_exact_supported_primitives() {
    let valid = r#""clock_basis":"c","scale":"seconds","ticks":9,"resolution_ticks":1,"uncertainty_ticks":2"#;
    for fields in [
        valid.replace("\"seconds\"", "\"Seconds\""),
        valid.replace("\"ticks\":9", "\"ticks\":-1"),
        valid.replace("\"ticks\":9", "\"ticks\":1.1"),
        valid.replace("\"ticks\":9", "\"ticks\":18446744073709551616"),
        valid.replace("\"resolution_ticks\":1", "\"resolution_ticks\":0"),
        valid.replace("\"clock_basis\":\"c\"", "\"clock_basis\":\"\""),
        valid.replace(",\"uncertainty_ticks\":2", ""),
        format!("{valid},\"ticks\":10"),
    ] {
        assert!(PreparedEpisodeRetrievalEvidence::new()
            .parse(instant(&fields).as_bytes())
            .is_err());
    }
    // An exact uncertain instant is retained even when a later interval calculation cannot represent it.
    let input = instant(&valid.replace("\"ticks\":9", "\"ticks\":0"));
    let parsed = PreparedEpisodeRetrievalEvidence::new()
        .parse(input.as_bytes())
        .unwrap();
    assert_eq!(
        parsed.items()[0].observed_at.unwrap().borrowed().interval(),
        Err(TemporalRelationError::IntervalOverflow)
    );
}
#[test]
fn byte_item_text_depth_and_node_limits_are_finite() {
    let mut parser = PreparedEpisodeRetrievalEvidence::new();
    assert_eq!(
        parser.parse(&vec![b' '; 4097]),
        Err(EpisodeRetrievalEvidenceError::ByteBound)
    );
    let items = format!(r#"{{"evidence":[{}]}}"#, ["{}"; 9].join(","));
    assert!(parser.parse(items.as_bytes()).is_err());
    let eight = format!(r#"{{"evidence":[{}]}}"#, ["{}"; 8].join(","));
    assert_eq!(parser.parse(eight.as_bytes()).unwrap().items().len(), 8);
    let long_text = format!(r#"{{"evidence":[{{"identity":"{}"}}]}}"#, "x".repeat(257));
    assert!(parser.parse(long_text.as_bytes()).is_err());
    let nested = format!(r#"{{"unknown":{}0{}}}"#, "[".repeat(8), "]".repeat(8));
    assert!(parser.parse(nested.as_bytes()).is_err());
    let many = format!(r#"{{"unknown":[{}]}}"#, ["0"; 513].join(","));
    assert!(parser.parse(many.as_bytes()).is_err());
}
#[test]
fn prepared_parser_allocates_nothing_on_valid_escaped_or_invalid_inputs() {
    let mut parser = PreparedEpisodeRetrievalEvidence::new();
    let ((valid, escaped, invalid), allocations) = allocation_probe::observe(|| {
        (
            parser.parse(ACTUAL),
            parser.parse(br#"{"evidence":[{"identity":"\u03bb\ud83d\ude42"}]}"#),
            parser.parse(br#"{"revision":-1}"#),
        )
    });
    assert!(valid.is_ok());
    assert!(escaped.is_ok());
    assert!(invalid.is_err());
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
}
