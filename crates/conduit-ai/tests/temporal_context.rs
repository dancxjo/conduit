use conduit_ai::{
    ClockBasis, EntityBoundary, TemporalContextRefusal, TemporalProvenance,
    TemporalRetrievalIntent, TemporalSource,
};
use conduit_core::TemporalRelation;

fn provenance() -> TemporalProvenance {
    TemporalProvenance {
        event_at: Some(100),
        valid_from: Some(200),
        valid_until: Some(500),
        observed_at: Some(300),
        recorded_at: Some(400),
        ingested_at: Some(900),
        retrieved_at: 990,
        reference_at: 1_000,
        clock_basis: ClockBasis::UnixEpochMilliseconds,
        uncertainty_millis: None,
    }
}

#[test]
fn old_observation_retrieved_now_remains_old() {
    let fact = provenance();
    assert_eq!(fact.age(TemporalSource::Observed), Ok(700));
    assert_eq!(fact.age(TemporalSource::Retrieved), Ok(10));
}

#[test]
fn exact_validity_boundaries_determine_duration() {
    assert_eq!(provenance().validity_duration(), Ok(Some(300)));
}

#[test]
fn future_event_is_a_typed_relation_but_not_an_age() {
    let mut fact = provenance();
    fact.event_at = Some(1_250);
    let relation = fact.relation(TemporalSource::Event).unwrap();
    assert_eq!(
        relation,
        TemporalRelation::Future {
            minimum_ticks: 250,
            maximum_ticks: 250,
        }
    );
    assert_eq!(
        fact.age(TemporalSource::Event),
        Err(TemporalContextRefusal::SourceAfterReference)
    );
}

#[test]
fn uncertain_age_remains_a_bounded_relation_not_a_scalar_guess() {
    let mut fact = provenance();
    fact.uncertainty_millis = Some(25);
    assert_eq!(
        fact.relation(TemporalSource::Observed),
        Ok(TemporalRelation::Past {
            minimum_ticks: 675,
            maximum_ticks: 725,
        })
    );
    assert_eq!(
        fact.age(TemporalSource::Observed),
        Err(TemporalContextRefusal::UncertainAge)
    );
}

#[test]
fn model_and_human_presentation_consume_the_same_canonical_relation() {
    let fact = provenance();
    let source = fact
        .canonical_source_instant(TemporalSource::Observed)
        .unwrap();
    let reference = conduit_presentation::TemporalReference {
        identity: "model-turn/reference".into(),
        instant: fact.canonical_reference_instant().unwrap(),
    };
    let presented = conduit_presentation::PresentationTemporalFact::new(
        "retrieved/fact".into(),
        conduit_presentation::PresentationTemporalRole::Observation,
        None,
        source,
        &reference,
    )
    .unwrap();
    assert_eq!(
        presented.relation,
        fact.relation(TemporalSource::Observed).unwrap()
    );
    assert_eq!(
        conduit_presentation::format_relative_time(&presented),
        "between 0 and 1 second ago"
    );
}

#[test]
fn missing_boundary_and_invalid_intervals_refuse_explicitly() {
    let mut fact = provenance();
    fact.event_at = None;
    assert_eq!(
        fact.relation(TemporalSource::Event),
        Err(TemporalContextRefusal::SourceUnavailable)
    );
    fact.valid_from = Some(600);
    assert_eq!(
        fact.validate(),
        Err(TemporalContextRefusal::ReversedValidityInterval)
    );
}

#[test]
fn retrieval_cannot_claim_knowledge_after_the_decision_instant() {
    let mut fact = provenance();
    fact.retrieved_at = 1_001;
    assert_eq!(
        fact.validate(),
        Err(TemporalContextRefusal::RetrievalAfterReference)
    );
}

#[test]
fn event_ordering_refuses_different_clock_bases() {
    let left = provenance();
    let mut right = provenance();
    right.clock_basis = ClockBasis::MonotonicMilliseconds {
        identity: "boot-7".into(),
    };
    assert_eq!(
        left.relation_to(TemporalSource::Event, &right, TemporalSource::Event),
        Err(TemporalContextRefusal::ClockBasisMismatch)
    );
}

#[test]
fn retrieval_intent_names_boundaries_and_rejects_reversed_windows() {
    assert!(TemporalRetrievalIntent::DurationSince {
        boundary: EntityBoundary::Born
    }
    .validate()
    .is_ok());
    assert_eq!(
        TemporalRetrievalIntent::EvidenceWithin { start: 20, end: 10 }.validate(),
        Err(TemporalContextRefusal::ReversedQueryWindow)
    );
}

#[test]
fn provenance_round_trip_preserves_distinct_temporal_facts() {
    let fact = provenance();
    let encoded = serde_json::to_string(&fact).unwrap();
    let decoded: TemporalProvenance = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, fact);
    assert_ne!(decoded.event_at, decoded.recorded_at);
    assert_ne!(decoded.recorded_at, decoded.ingested_at);
    assert_ne!(decoded.ingested_at, Some(decoded.retrieved_at));
}
