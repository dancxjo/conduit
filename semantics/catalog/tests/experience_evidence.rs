#[path = "common/experience_evidence.rs"]
mod common;
use common::*;
use conduit_human::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use conduit_semantic_catalog::BoundedExperienceEvidenceProjection;

#[test]
fn projection_preserves_explicit_epistemic_content_and_selection_order() {
    let observed = observation("observation");
    let remembered = item(
        "memory",
        ExperienceOrigin::Remembered,
        ExperienceTemporalRole::Historical,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        b"selected recollection",
    );
    let derived = item(
        "model",
        ExperienceOrigin::ModelDerived,
        ExperienceTemporalRole::Stale,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        b"uncertain derived interpretation",
    );
    let missing = item(
        "missing",
        ExperienceOrigin::Observation,
        ExperienceTemporalRole::Recent,
        ExperienceAvailability::NotObserved,
        7,
        "provenance/7",
        b"",
    );
    let unavailable = item(
        "unavailable",
        ExperienceOrigin::HumanReported,
        ExperienceTemporalRole::Stale,
        ExperienceAvailability::SourceUnavailable,
        7,
        "provenance/7",
        b"",
    );
    let ids = ["memory", "missing", "model", "observation", "unavailable"];
    let input = request(
        &ids,
        vec![
            observed.clone(),
            derived.clone(),
            remembered.clone(),
            missing.clone(),
            unavailable.clone(),
        ],
    );
    let expected = ExperienceEvidenceContext::new(
        items(vec![remembered, missing, derived, observed, unavailable]),
        selection(&ids),
    )
    .unwrap();
    let mut projection = BoundedExperienceEvidenceProjection::prepare();
    let output = projection.execute(&encode(input)).unwrap();
    assert_eq!(output, encode(expected));
    let decoded = ExperienceEvidenceContext::from_structured(
        conduit_core::StructuredInfoValue::from_canonical_bytes(output).unwrap(),
    )
    .unwrap();
    assert_eq!(decoded.items().get().as_slice().len(), ids.len());
}

#[test]
fn exact_selection_rejects_mismatch_duplicate_extra_missing_and_stale_evidence() {
    use ExperienceEvidenceRefusal::*;
    let cases = [
        (
            request(&["a", "a"], vec![observation("a")]),
            DuplicateSelection,
        ),
        (
            request(&["a"], vec![observation("a"), observation("a")]),
            DuplicateEvidence,
        ),
        (request(&["a"], vec![observation("b")]), UnselectedEvidence),
        (
            request(&["a", "b"], vec![observation("a")]),
            MissingEvidence,
        ),
        (
            request(
                &["a"],
                vec![item(
                    "a",
                    ExperienceOrigin::Observation,
                    ExperienceTemporalRole::Current,
                    ExperienceAvailability::Present,
                    6,
                    "provenance/7",
                    b"old",
                )],
            ),
            StaleRevision,
        ),
        (
            request(
                &["a"],
                vec![item(
                    "a",
                    ExperienceOrigin::Observation,
                    ExperienceTemporalRole::Current,
                    ExperienceAvailability::Present,
                    8,
                    "provenance/7",
                    b"future",
                )],
            ),
            FutureRevision,
        ),
        (
            request(
                &["a"],
                vec![item(
                    "a",
                    ExperienceOrigin::Observation,
                    ExperienceTemporalRole::Current,
                    ExperienceAvailability::Present,
                    7,
                    "different-provenance",
                    b"mismatch",
                )],
            ),
            ProvenanceMismatch,
        ),
    ];
    let mut projection = BoundedExperienceEvidenceProjection::prepare();
    for (input, refusal) in cases {
        assert_eq!(projection.execute(&encode(input)), Err(refusal));
        assert!(projection.output().is_empty());
    }
}

#[test]
fn epistemic_contradictions_are_refused_instead_of_promoted_to_observation() {
    for invalid in [
        item(
            "a",
            ExperienceOrigin::Remembered,
            ExperienceTemporalRole::Current,
            ExperienceAvailability::Present,
            7,
            "provenance/7",
            b"memory is not now",
        ),
        item(
            "a",
            ExperienceOrigin::Observation,
            ExperienceTemporalRole::Current,
            ExperienceAvailability::SourceUnavailable,
            7,
            "provenance/7",
            b"not observed",
        ),
        item(
            "a",
            ExperienceOrigin::ModelDerived,
            ExperienceTemporalRole::Current,
            ExperienceAvailability::NotObserved,
            7,
            "provenance/7",
            b"invented evidence",
        ),
    ] {
        assert_eq!(
            BoundedExperienceEvidenceProjection::prepare()
                .execute(&encode(request(&["a"], vec![invalid]))),
            Err(ExperienceEvidenceRefusal::InvalidEpistemicCombination)
        );
    }
}

#[test]
fn native_construction_and_runtime_admission_are_finite_without_truncation() {
    assert!(ExperienceEvidenceIdentity::new("x".repeat(129)).is_err());
    assert!(ExperienceEvidenceIdentity::new("".into()).is_err());
    assert!(ExperienceEvidenceContent::new(BoundedBytes::new(&[0; 2049]).unwrap()).is_err());
    assert!(
        BoundedSequence::<ExperienceEvidenceItem, 8>::try_from_iter(vec![observation("a"); 9])
            .is_err()
    );
    let ids = ["a", "b", "c", "d", "e", "f", "g", "h"];
    let maximum = request(
        &ids,
        ids.iter()
            .map(|id| {
                item(
                    id,
                    ExperienceOrigin::Observation,
                    ExperienceTemporalRole::Current,
                    ExperienceAvailability::Present,
                    7,
                    "provenance/7",
                    &vec![0xff; 2048],
                )
            })
            .collect(),
    );
    let input = encode(maximum);
    let mut projection = BoundedExperienceEvidenceProjection::prepare();
    let output = projection.execute(&input).unwrap();
    assert!(!output.is_empty());
    assert!(output.len() <= conduit_semantic_catalog::EXPERIENCE_EVIDENCE_MAX_BYTES);
    assert_eq!(
        projection.execute(&vec![0; 65_537]),
        Err(ExperienceEvidenceRefusal::ByteBound)
    );
    assert_eq!(
        projection.execute(b"not canonical"),
        Err(ExperienceEvidenceRefusal::Malformed)
    );
    let empty = encode(request(&[], vec![]));
    assert!(projection.execute(&empty).is_ok());
}

#[test]
fn canonical_boundary_rechecks_refined_byte_and_identity_bounds() {
    let input = encode(request(&["a"], vec![observation("a")]));
    let replace_leaf = |needle: &[u8], replacement: &[u8]| {
        let at = input
            .windows(needle.len())
            .position(|window| window == needle)
            .unwrap();
        let mut bytes = input[..at - 4].to_vec();
        bytes.extend_from_slice(&(replacement.len() as u32).to_le_bytes());
        bytes.extend_from_slice(replacement);
        bytes.extend_from_slice(&input[at + needle.len()..]);
        bytes
    };
    let mut projection = BoundedExperienceEvidenceProjection::prepare();
    assert_eq!(
        projection.execute(&replace_leaf(b"observed evidence", &vec![0; 2049])),
        Err(ExperienceEvidenceRefusal::ByteBound)
    );
    assert_eq!(
        projection.execute(&replace_leaf(b"source/utterance", &[b'x'; 129])),
        Err(ExperienceEvidenceRefusal::IdentityBound)
    );
    let valid = projection.execute(&input).unwrap();
    assert!(
        !valid.is_empty(),
        "refusal leaves no hidden pending context"
    );
}

#[test]
fn provenance_references_remain_data_and_duplicate_sources_are_refused() {
    let resource = resource_source();
    let memory = ExperienceEvidenceSource::memory_record(identity("memory/record/7")).unwrap();
    let item = with_sources(observation("a"), vec![resource.clone(), memory]);
    let expected = encode(
        ExperienceEvidenceContext::new(items(vec![item.clone()]), selection(&["a"])).unwrap(),
    );
    let mut projection = BoundedExperienceEvidenceProjection::prepare();
    assert_eq!(
        projection
            .execute(&encode(request(&["a"], vec![item])))
            .unwrap(),
        expected
    );
    let duplicated = with_sources(observation("a"), vec![resource.clone(), resource]);
    assert_eq!(
        projection.execute(&encode(request(&["a"], vec![duplicated]))),
        Err(ExperienceEvidenceRefusal::SourceBound)
    );
}

#[path = "common/experience_temporal.rs"]
mod temporal;
#[test]
fn native_temporal_context_is_preserved_without_resolving_clock_identities() {
    let value = with_sources(
        item(
            "time",
            ExperienceOrigin::Observation,
            ExperienceTemporalRole::Indeterminate,
            ExperienceAvailability::Present,
            7,
            "provenance/7",
            b"uncertain observation",
        ),
        vec![temporal::source()],
    );
    let input = request(&["time"], vec![value.clone()]).encode().unwrap();
    let expected = ExperienceEvidenceContext::new(items(vec![value]), selection(&["time"]))
        .unwrap()
        .encode()
        .unwrap();
    let mut projection = BoundedExperienceEvidenceProjection::prepare();
    assert_eq!(projection.execute(&input).unwrap(), expected);
    // Clock-basis mismatch is explicit provenance, not authority to invent a
    // relationship or reject an honestly indeterminate observation.
}
#[test]
fn duplicate_temporal_context_and_invalid_time_or_policy_refuse() {
    let source = temporal::source();
    let duplicate = request(
        &["time"],
        vec![with_sources(
            observation("time"),
            vec![source.clone(), source.clone()],
        )],
    )
    .encode()
    .unwrap();
    let mut projection = BoundedExperienceEvidenceProjection::prepare();
    assert_eq!(
        projection.execute(&duplicate),
        Err(ExperienceEvidenceRefusal::SourceBound)
    );
    let valid = request(
        &["time"],
        vec![with_sources(observation("time"), vec![source])],
    )
    .encode()
    .unwrap();
    for (field, value) in [
        ("maximum_current_age_ticks", 50),
        ("resolution_ticks", 0),
        ("uncertainty_ticks", 201),
    ] {
        let mut forged = valid.clone();
        temporal::set_u64(&mut forged, field, value);
        assert_eq!(
            projection.execute(&forged),
            Err(ExperienceEvidenceRefusal::Malformed)
        );
        assert!(projection.output().is_empty());
    }
    assert!(projection.execute(&valid).is_ok());
}
