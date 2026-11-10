use super::*;
use crate::*;
use conduit_core::{SignId, TemporalInstant, TemporalScale};
use conduit_human::*;
use conduit_plot::rust_binding::NativeRustBinding;

fn at(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/pete-experience".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn observed<T>(value: T, sign: &str, ticks: u64) -> SituationInput<T> {
    SituationInput::Observed(Observed {
        value,
        sign_id: SignId::from(sign),
        observed_at: at(ticks),
    })
}

fn experience(inputs: PeteExperienceInputs) -> CurrentExperience {
    let mut current = CurrentExperience::new(
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
            maximum_relationships: 4,
            maximum_item_bytes: 2048,
            maximum_encoded_bytes: 8192,
            maximum_conflict_alternatives: 2,
            maximum_identity_bytes: 128,
        },
        at(200),
        ExperienceTemporalPolicy::new(20, 50).unwrap(),
    )
    .unwrap();
    for item in inputs.items {
        current.try_admit(item).unwrap();
    }
    for relation in inputs.relationships {
        current.relate(relation).unwrap();
    }
    current
}

#[test]
fn situation_adapter_preserves_stale_and_conflicting_sources() {
    let situation = select_pete_situation(
        at(200),
        20,
        observed(
            conduit_robotics::BatteryObservation::new(500, 13000).unwrap(),
            "battery/old",
            100,
        ),
        SituationInput::Contradictory {
            first: Observed {
                value: "door open".into(),
                sign_id: "human/1".into(),
                observed_at: at(195),
            },
            second: Observed {
                value: "door closed".into(),
                sign_id: "human/2".into(),
                observed_at: at(196),
            },
        },
        SituationInput::Unavailable {
            source_identity: "body/source".into(),
        },
    )
    .unwrap();
    let items =
        pete_situation_experiences(&situation, ExperienceTemporalPolicy::new(20, 50).unwrap())
            .unwrap();
    assert_eq!(items.relationships.len(), 1);
    assert_eq!(
        items.relationships[0].kind,
        ExperienceRelationKind::Contradicts
    );
    assert_eq!(items.items.len(), 4);
    assert_eq!(items.items[0].temporal_role, ExperienceTemporalRole::Stale);
    assert_eq!(items.items[1].origin, ExperienceOrigin::HumanReported);
    assert_ne!(items.items[1].sources, items.items[2].sources);
    assert_eq!(items.items[1].certainty, ExperienceCertainty::Uncertain);
    assert_eq!(
        items.items[3].availability,
        ExperienceAvailability::SourceUnavailable
    );
    assert!(items.items[3].encoded_content.is_empty());
    let current = experience(items);
    let request = project_pete_experience_evidence(&current, 7, "provenance/7").unwrap();
    let encoded = request.encode().unwrap();
    let mut projection = conduit_semantic_catalog::BoundedExperienceEvidenceProjection::prepare();
    let context = ExperienceEvidenceContext::decode(projection.execute(&encoded).unwrap()).unwrap();
    assert_eq!(context.items().get().len(), 4);
    assert_eq!(
        *context.items().get()[0].temporal_role(),
        ExperienceTemporalRole::Stale
    );
    assert_eq!(*context.selection().revision(), 7);
    assert_eq!(context.items().get()[1].observed_at(), &known_at(195));
    assert_eq!(
        context.items().get()[3].observed_at(),
        &ExperienceEvidenceInstant::Unknown
    );
}

fn known_at(ticks: u64) -> ExperienceEvidenceInstant {
    ExperienceEvidenceInstant::known(
        conduit_time::NativeTemporalInstant::try_from(at(ticks)).unwrap(),
    )
    .unwrap()
}

#[test]
fn explicit_projection_keeps_memory_separate_from_present_observation() {
    let situation = select_pete_situation(
        at(200),
        20,
        SituationInput::Missing,
        observed("Is the hallway still lit?".into(), "human/2", 195),
        SituationInput::Missing,
    )
    .unwrap();
    let mut items =
        pete_situation_experiences(&situation, ExperienceTemporalPolicy::new(20, 50).unwrap())
            .unwrap();
    items.items.push(
        recollected_experience(
            "memory/episode1",
            &SelectedRecollection {
                record_id: "episode/1".into(),
                content_kind: "value/text".into(),
                canonical_content: b"The hallway light was on".to_vec(),
                occurred_at: at(100),
                recorded_at: at(110),
                original_sources: vec![ExperienceSourceRef::Sign("light/1".into())],
                certainty: ExperienceCertainty::Uncertain,
            },
        )
        .unwrap(),
    );
    let current = experience(items);
    let request = project_pete_experience_evidence(&current, 2, "provenance/2").unwrap();
    let encoded = request.encode().unwrap();
    let mut projection = conduit_semantic_catalog::BoundedExperienceEvidenceProjection::prepare();
    let context = ExperienceEvidenceContext::decode(projection.execute(&encoded).unwrap()).unwrap();
    let evidence = context.items().get();
    assert_eq!(*evidence[0].origin(), ExperienceOrigin::HumanReported);
    assert_eq!(*evidence[1].origin(), ExperienceOrigin::Remembered);
    assert_eq!(
        *evidence[1].temporal_role(),
        ExperienceTemporalRole::Historical
    );
    assert_eq!(evidence[0].observed_at(), &known_at(195));
    assert_eq!(
        evidence[0].recorded_at(),
        &ExperienceEvidenceInstant::Unknown
    );
    assert_eq!(evidence[1].observed_at(), &known_at(100));
    assert_eq!(evidence[1].recorded_at(), &known_at(110));
    assert_eq!(evidence[1].sources().get().len(), 2);
    assert!(matches!(
        project_pete_experience_evidence(&current, 2, ""),
        Err(ExperienceEvidenceRefusal::IdentityBound)
    ));
}

#[test]
fn selected_autobiography_keeps_original_model_kind_and_exact_execution_provenance() {
    let record = RetainedExperience {
        sequence: 0,
        candidate: ExperienceCandidate {
            identity: "episode/1".into(),
            kind: ExperienceKind::ModelDerived,
            content: b"The light may be on".to_vec(),
            provenance: vec![ExperienceProvenance {
                source_identity: "model/result/1".into(),
                event_at_millis: 100,
                recorded_at_millis: 110,
                body_identity: "body/pete".into(),
                host_identity: Some("host/1".into()),
                boot_identity: Some("boot/1".into()),
                plan_identity: Some("plan/1".into()),
                play_identity: Some("play/1".into()),
            }],
            sensitivity: Sensitivity::LocalPrivate,
            supersedes: None,
            explicit_remember: true,
        },
    };
    let item = pete_retained_experience_input(&record).unwrap();
    assert_eq!(item.origin, ExperienceOrigin::Remembered);
    assert_eq!(item.temporal_role, ExperienceTemporalRole::Historical);
    assert_eq!(item.content_kind.as_str(), "pete/remembered-model-result@1");
    let candidate: ExperienceCandidate = serde_json::from_slice(&item.encoded_content).unwrap();
    assert_eq!(candidate, record.candidate);
    assert_eq!(
        item.sources[0],
        ExperienceSourceRef::MemoryRecord {
            record_id: "episode/1".into()
        }
    );
    assert_eq!(item.observed_at.unwrap().ticks, 100);
    assert_eq!(item.recorded_at.unwrap().ticks, 110);
    let mut malformed = record;
    malformed.candidate.provenance[0].event_at_millis = 111;
    assert_eq!(
        pete_retained_experience_input(&malformed),
        Err(PeteExperienceRefusal::InvalidFact)
    );
}

#[test]
fn malformed_fact_does_not_produce_partial_experience_inputs() {
    let mut situation = select_pete_situation(
        at(200),
        20,
        observed(
            conduit_robotics::BatteryObservation::new(500, 13000).unwrap(),
            "battery/1",
            195,
        ),
        observed("hello".into(), "human/1", 196),
        SituationInput::Missing,
    )
    .unwrap();
    let original_kind = situation.battery.value_kind.clone();
    situation.battery.value_kind = "unrelated/value-kind".into();
    assert_eq!(
        pete_situation_experiences(&situation, ExperienceTemporalPolicy::new(20, 50).unwrap()),
        Err(PeteExperienceRefusal::InvalidFact)
    );
    situation.battery.value_kind = original_kind;
    situation.utterance.provenance[0].relation = conduit_core::TemporalRelation::Future {
        minimum_ticks: 1,
        maximum_ticks: 1,
    };
    assert_eq!(
        pete_situation_experiences(&situation, ExperienceTemporalPolicy::new(20, 50).unwrap()),
        Err(PeteExperienceRefusal::InvalidFact)
    );
}
