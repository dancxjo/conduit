extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use conduit_core::SignId;

use conduit_presentation::*;

fn instant(ticks: u64, uncertainty_ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/test-1".into(),
        resolution_ticks: 1,
        uncertainty_ticks,
    }
}

fn presentation(
    references: Vec<TemporalReference>,
    facts: Vec<PresentationTemporalFact>,
) -> Result<Presentation, PresentationError> {
    Presentation::new_with_semantics_and_temporal(
        1,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![SignId::from("sign/observed")],
        },
        vec![PresentationSubject {
            identity: "subject/event".into(),
            role: PresentationRole::Diagnostic,
            label: "Event".into(),
            accessibility_name: "Observed event".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        references,
        facts,
    )
}

#[test]
fn checked_relations_keep_overlap_distinct_from_exact_present() {
    assert_eq!(
        instant(10, 0).relation_to(&instant(20, 0)),
        Ok(TemporalRelation::Past {
            minimum_ticks: 10,
            maximum_ticks: 10,
        })
    );
    assert_eq!(
        instant(30, 2).relation_to(&instant(20, 1)),
        Ok(TemporalRelation::Future {
            minimum_ticks: 7,
            maximum_ticks: 13,
        })
    );
    assert_eq!(
        instant(20, 0).relation_to(&instant(20, 0)),
        Ok(TemporalRelation::Present)
    );
    assert_eq!(
        instant(20, 1).relation_to(&instant(20, 1)),
        Ok(TemporalRelation::Indeterminate)
    );
}

#[test]
fn incomparable_invalid_and_overflowing_instants_refuse() {
    let mut other_basis = instant(20, 0);
    other_basis.clock_basis = "clock/test-2".into();
    assert_eq!(
        instant(10, 0).relation_to(&other_basis),
        Err(TemporalRelationError::Incomparable)
    );
    let mut other_scale = instant(20, 0);
    other_scale.scale = TemporalScale::Nanoseconds;
    assert_eq!(
        instant(10, 0).relation_to(&other_scale),
        Err(TemporalRelationError::Incomparable)
    );
    let mut invalid = instant(10, 0);
    invalid.resolution_ticks = 0;
    assert_eq!(
        invalid.relation_to(&instant(20, 0)),
        Err(TemporalRelationError::InvalidInstant)
    );
    assert_eq!(
        instant(0, 1).relation_to(&instant(20, 0)),
        Err(TemporalRelationError::IntervalOverflow)
    );
    assert_eq!(
        instant(u64::MAX, 1).relation_to(&instant(20, 0)),
        Err(TemporalRelationError::IntervalOverflow)
    );
}

#[test]
fn temporal_facts_are_identity_bearing_validated_and_linearized() {
    let reference = TemporalReference {
        identity: "reference/now".into(),
        instant: instant(20, 1),
    };
    let fact = PresentationTemporalFact::new(
        "subject/event".into(),
        PresentationTemporalRole::Observation,
        Some(SignId::from("sign/observed")),
        instant(10, 1),
        &reference,
    )
    .unwrap();
    let value = presentation(vec![reference.clone()], vec![fact.clone()]).unwrap();
    let without_temporal = presentation(vec![], vec![]).unwrap();
    assert_ne!(value.identity, without_temporal.identity);
    let linear = render_linear_presentation(&value).unwrap();
    assert!(linear
        .lines
        .iter()
        .any(|line| line.starts_with("TEMPORAL_REFERENCE")));
    let relative_index = linear
        .lines
        .iter()
        .position(|line| line.starts_with("RELATIVE_TIME"))
        .unwrap();
    let fact_index = linear
        .lines
        .iter()
        .position(|line| line.starts_with("TEMPORAL_FACT"))
        .unwrap();
    assert_eq!(relative_index + 1, fact_index);
    assert!(linear.lines[relative_index].contains("value=\"between 0 and 1 second ago\""));
    assert!(linear.lines[fact_index].contains("sign=sign/observed"));
    assert!(linear.lines[fact_index].contains("source_clock_basis=\"clock/test-1\""));
    assert!(linear.lines[fact_index].contains("source_resolution=1 source_uncertainty=1"));

    let mut stale = value.clone();
    stale.temporal_facts[0].relation = TemporalRelation::Future {
        minimum_ticks: 1,
        maximum_ticks: 1,
    };
    assert_eq!(
        stale.validate(),
        Err(PresentationError::InvalidTemporalRelation)
    );

    let mut unknown_subject = fact.clone();
    unknown_subject.subject = "subject/missing".into();
    assert_eq!(
        presentation(vec![reference.clone()], vec![unknown_subject]),
        Err(PresentationError::UnknownTemporalSubject)
    );
    let mut unknown_sign = fact;
    unknown_sign.sign_id = Some(SignId::from("sign/missing"));
    assert_eq!(
        presentation(vec![reference.clone()], vec![unknown_sign]),
        Err(PresentationError::UnknownTemporalSign)
    );
    let mut unknown_reference = value.temporal_facts[0].clone();
    unknown_reference.reference = "reference/missing".into();
    assert_eq!(
        presentation(vec![reference], vec![unknown_reference]),
        Err(PresentationError::UnknownTemporalReference)
    );
}

#[test]
fn changing_only_the_reference_recomputes_truth_not_observation_evidence() {
    let source = instant(100, 0);
    let earlier = TemporalReference {
        identity: "reference/earlier".into(),
        instant: instant(80, 0),
    };
    let later = TemporalReference {
        identity: "reference/later".into(),
        instant: instant(120, 0),
    };
    let against_earlier = PresentationTemporalFact::new(
        "subject/event".into(),
        PresentationTemporalRole::Observation,
        Some(SignId::from("sign/observed")),
        source.clone(),
        &earlier,
    )
    .unwrap();
    let against_later = PresentationTemporalFact::new(
        "subject/event".into(),
        PresentationTemporalRole::Observation,
        Some(SignId::from("sign/observed")),
        source,
        &later,
    )
    .unwrap();

    assert_eq!(against_earlier.subject, against_later.subject);
    assert_eq!(against_earlier.role, against_later.role);
    assert_eq!(against_earlier.sign_id, against_later.sign_id);
    assert_eq!(against_earlier.source, against_later.source);
    assert_eq!(
        against_earlier.relation,
        TemporalRelation::Future {
            minimum_ticks: 20,
            maximum_ticks: 20,
        }
    );
    assert_eq!(
        against_later.relation,
        TemporalRelation::Past {
            minimum_ticks: 20,
            maximum_ticks: 20,
        }
    );

    let earlier_presentation = presentation(vec![earlier], vec![against_earlier.clone()]).unwrap();
    let later_presentation = presentation(vec![later], vec![against_later.clone()]).unwrap();
    assert_eq!(earlier_presentation.revision, later_presentation.revision);
    assert_ne!(earlier_presentation.identity, later_presentation.identity);
    assert_eq!(
        earlier_presentation.temporal_facts[0].source,
        later_presentation.temporal_facts[0].source
    );
    let earlier_linear = render_linear_presentation(&earlier_presentation).unwrap();
    let later_linear = render_linear_presentation(&later_presentation).unwrap();
    assert!(earlier_linear
        .lines
        .iter()
        .any(|line| line.contains("value=\"in 0 to 1 second\"")));
    assert!(later_linear
        .lines
        .iter()
        .any(|line| line.contains("value=\"between 0 and 1 second ago\"")));
    assert_eq!(against_earlier.source, against_later.source);
    assert_eq!(against_earlier.subject, against_later.subject);
    assert_eq!(against_earlier.role, against_later.role);
    assert_eq!(against_earlier.sign_id, against_later.sign_id);
}

#[test]
fn temporal_collections_and_reference_identities_are_finite() {
    let reference = TemporalReference {
        identity: "reference/duplicate".into(),
        instant: instant(20, 0),
    };
    assert_eq!(
        presentation(vec![reference.clone(), reference], vec![]),
        Err(PresentationError::DuplicateTemporalReference)
    );
    let references = (0..=MAX_TEMPORAL_REFERENCES)
        .map(|index| TemporalReference {
            identity: format!("reference/{index}"),
            instant: instant(index as u64 + 1, 0),
        })
        .collect();
    assert_eq!(
        presentation(references, vec![]),
        Err(PresentationError::TooManyTemporalReferences)
    );
    let reference = TemporalReference {
        identity: "reference/now".into(),
        instant: instant(20, 0),
    };
    let fact = PresentationTemporalFact::new(
        "subject/event".into(),
        PresentationTemporalRole::Event,
        None,
        instant(10, 0),
        &reference,
    )
    .unwrap();
    assert_eq!(
        presentation(
            vec![reference],
            vec![fact; MAX_PRESENTATION_TEMPORAL_FACTS + 1]
        ),
        Err(PresentationError::TooManyTemporalFacts)
    );
}
