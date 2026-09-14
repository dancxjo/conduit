use super::*;
use conduit_ai::{TemporalEvidenceSelection, TemporalRetrievalIntent};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
};

fn candidate(identity: &str, kind: ExperienceKind) -> ExperienceCandidate {
    ExperienceCandidate {
        identity: identity.into(),
        kind,
        content: b"bounded fact".to_vec(),
        provenance: vec![ExperienceProvenance {
            source_identity: format!("sign/{identity}"),
            event_at_millis: 10,
            recorded_at_millis: 11,
            body_identity: "body/pete".into(),
            host_identity: Some("host/brainstem".into()),
            boot_identity: Some("boot/7".into()),
            plan_identity: Some("plan/3".into()),
            play_identity: Some("play/3".into()),
        }],
        sensitivity: Sensitivity::LocalPrivate,
        supersedes: None,
        explicit_remember: true,
    }
}

#[test]
fn distinct_fact_classes_retain_exact_provenance() {
    let mut memory = BoundedAutobiography::new(8).unwrap();
    for (identity, kind) in [
        ("observation", ExperienceKind::ObservedSign),
        ("human", ExperienceKind::HumanStatement),
        ("model", ExperienceKind::ModelDerived),
        ("request", ExperienceKind::ActionRequest),
        ("effect", ExperienceKind::EffectResult),
    ] {
        memory.retain(candidate(identity, kind), true).unwrap();
    }
    let records = memory.records(true).unwrap();
    assert_eq!(records.len(), 5);
    assert_ne!(records[0].candidate.kind, records[2].candidate.kind);
    assert_eq!(
        records[0].candidate.provenance[0].play_identity.as_deref(),
        Some("play/3")
    );
}

#[test]
fn corrections_append_and_pressure_provider_loss_and_authority_are_explicit() {
    let mut memory = BoundedAutobiography::new(2).unwrap();
    memory
        .retain(candidate("original", ExperienceKind::HumanStatement), true)
        .unwrap();
    let mut correction = candidate("correction", ExperienceKind::OperatorCorrection);
    correction.supersedes = Some("original".into());
    memory.retain(correction, true).unwrap();
    assert_eq!(memory.records(true).unwrap().len(), 2);
    assert_eq!(
        memory.retain(candidate("third", ExperienceKind::EffectResult), true),
        Err(MemoryRefusal::CapacityPressure)
    );
    assert_eq!(
        memory.forget("original", false),
        Err(MemoryRefusal::AuthorityRefused)
    );
    memory.set_provider_available(false);
    assert_eq!(
        memory.records(true),
        Err(MemoryRefusal::ProviderUnavailable)
    );
}

#[test]
fn canonical_memory_form_is_checked_and_host_neutral() {
    let source = include_str!("../../../forms/pete-memory/main.conduit");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_pete_memory_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "pete-memory", &profile).unwrap();
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        PETE_MEMORY_RETAIN_KIND
    );
}

#[test]
fn temporal_queries_delegate_to_the_generic_temporal_contract() {
    let mut memory = BoundedAutobiography::new(4).unwrap();
    let mut early = candidate("early", ExperienceKind::ObservedSign);
    early.provenance[0].event_at_millis = 10;
    early.provenance[0].recorded_at_millis = 11;
    let mut late = candidate("late", ExperienceKind::HumanStatement);
    late.provenance[0].event_at_millis = 30;
    late.provenance[0].recorded_at_millis = 31;
    memory.retain(late, true).unwrap();
    memory.retain(early, true).unwrap();
    assert_eq!(
        memory.select_temporal(100, &TemporalRetrievalIntent::EarliestEvidence, true),
        Ok(TemporalEvidenceSelection::Selected {
            identities: vec!["early".into()]
        })
    );
    assert_eq!(
        memory.select_temporal(100, &TemporalRetrievalIntent::LatestEvidence, true),
        Ok(TemporalEvidenceSelection::Selected {
            identities: vec!["late".into()]
        })
    );
    assert_eq!(
        memory.select_temporal(
            100,
            &TemporalRetrievalIntent::EvidenceWithin { start: 20, end: 40 },
            true
        ),
        Ok(TemporalEvidenceSelection::Selected {
            identities: vec!["late".into()]
        })
    );
}
