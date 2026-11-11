use conduit_core::{kind_id, TemporalInstant, TemporalScale};
use conduit_human::*;

fn at(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/integration".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}
fn limits() -> ExperienceLimits {
    ExperienceLimits {
        maximum_items: 8,
        maximum_current_items: 8,
        maximum_recent_items: 8,
        maximum_stale_items: 8,
        maximum_historical_items: 8,
        maximum_items_per_domain: 8,
        maximum_model_derived_items: 8,
        maximum_selected_memory_items: 8,
        maximum_source_refs: 4,
        maximum_relationships: 8,
        maximum_item_bytes: 2048,
        maximum_encoded_bytes: 65536,
        maximum_conflict_alternatives: 2,
        maximum_identity_bytes: 128,
    }
}
fn item(id: &str) -> ExperienceItem {
    ExperienceItem {
        id: id.into(),
        domain: ExperienceDomain::BodyState,
        content_kind: kind_id("test/body@1"),
        encoded_content: b"50%".to_vec(),
        origin: ExperienceOrigin::Observation,
        temporal_role: ExperienceTemporalRole::Current,
        availability: ExperienceAvailability::Present,
        certainty: ExperienceCertainty::Certain,
        observed_at: Some(at(100)),
        recorded_at: None,
        sources: vec![ExperienceSourceRef::Sign("sensor/1".into())],
    }
}
fn batch(evidence: ExperienceEvidenceRequest, reference: u64) -> ExperienceIntegrationInput {
    ExperienceIntegrationInput::new(
        evidence,
        ExperienceTemporalPolicy::new(5, 20).unwrap(),
        conduit_time::NativeTemporalInstant::try_from(at(reference)).unwrap(),
    )
    .unwrap()
}

#[test]
fn batch_admission_preserves_current_historical_and_derived_provenance() {
    let mut historical = item("historical");
    historical.origin = ExperienceOrigin::Remembered;
    historical.domain = ExperienceDomain::Recollection;
    historical.temporal_role = ExperienceTemporalRole::Historical;
    historical.observed_at = Some(at(1));
    historical.recorded_at = Some(at(10));
    historical.sources = vec![ExperienceSourceRef::MemoryRecord {
        record_id: "episode/1".into(),
    }];
    let mut derived = item("derived");
    derived.origin = ExperienceOrigin::ModelDerived;
    derived.certainty = ExperienceCertainty::Uncertain;
    derived.sources = vec![ExperienceSourceRef::ImplementationRun {
        implementation_id: "implementation/model@1".into(),
        provider_instance_id: "provider/model/1".into(),
        artifact_id: "artifact/model@1".into(),
        run_id: "run/model/1".into(),
    }];
    let mut report = item("report");
    report.origin = ExperienceOrigin::HumanReported;
    report.sources = vec![
        ExperienceSourceRef::HumanStatement {
            statement_id: "human/1".into(),
        },
        ExperienceSourceRef::Source {
            source_id: "human".into(),
        },
    ];
    let items = vec![item("observed"), historical, derived, report];
    let relations = vec![ExperienceRelation {
        subject_id: "observed".into(),
        object_id: "historical".into(),
        kind: ExperienceRelationKind::Contradicts,
    }];
    let evidence = prepare_experience_evidence(&items, &relations, 4, "sources/4").unwrap();
    let current = integrate_experience_evidence(&batch(evidence.clone(), 100), limits()).unwrap();
    assert_eq!(current.items(), items);
    assert_eq!(current.relationships(), relations);
    assert_eq!(
        project_experience_evidence(&current, 4, "sources/4").unwrap(),
        evidence
    );
}

#[test]
fn batch_admission_refuses_false_freshness_and_mismatched_revision() {
    let evidence = prepare_experience_evidence(&[item("observed")], &[], 1, "sources/1").unwrap();
    assert_eq!(
        integrate_experience_evidence(&batch(evidence.clone(), 200), limits()),
        Err(ExperienceIntegrationError::Admission(
            ExperienceRefusal::TemporalRoleMismatch
        ))
    );

    let selection = evidence.selection();
    let incorrect = ExperienceEvidenceSelection::new(
        selection.item_identities().clone(),
        selection.provenance_revision().clone(),
        selection.relationships().clone(),
        2,
    )
    .unwrap();
    let input = ExperienceEvidenceRequest::new(evidence.evidence().clone(), incorrect).unwrap();
    assert_eq!(
        integrate_experience_evidence(&batch(input, 100), limits()),
        Err(ExperienceIntegrationError::Evidence(
            ExperienceEvidenceRefusal::StaleRevision
        ))
    );
}
