use conduit_ai::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
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
fn identity(value: &str) -> TemporalEvidenceIdentity {
    TemporalEvidenceIdentity::new(value.into()).unwrap()
}
fn candidate(id: &str, time: u64) -> TemporalEvidenceCandidate {
    TemporalEvidenceCandidate::new(
        Some(EntityBoundary::Created),
        identity(id),
        TemporalProvenance {
            event_at: Some(time),
            valid_from: Some(time),
            valid_until: Some(time + 10),
            observed_at: Some(time),
            recorded_at: Some(time + 1),
            ingested_at: None,
            retrieved_at: 100,
            reference_at: 100,
            clock_basis: ClockBasis::monotonic_milliseconds(
                ClockIdentity::new("clock/script".into()).unwrap(),
            )
            .unwrap(),
            uncertainty_millis: Some(1),
        },
        TemporalSource::Event,
        Some(TransitionDirection::IntoState),
        TemporalValidity::Historical,
    )
    .unwrap()
}
fn window(candidates: Vec<TemporalEvidenceCandidate>, complete: bool) -> TemporalEvidenceWindow {
    TemporalEvidenceWindow::new(
        TemporalEvidenceWindowCandidates::new(BoundedSequence::try_from_iter(candidates).unwrap())
            .unwrap(),
        complete,
        identity("window/retained/1"),
        TemporalReference {
            reference_at: 100,
            clock_basis: ClockBasis::monotonic_milliseconds(
                ClockIdentity::new("clock/script".into()).unwrap(),
            )
            .unwrap(),
        },
    )
    .unwrap()
}
fn inputs() -> Vec<TemporalEvidenceCandidate> {
    vec![
        candidate("later", 30),
        candidate("earlier/b", 10),
        candidate("earlier/a", 10),
    ]
}
fn selected(outcome: &TemporalEvidenceOutcome) -> Option<Vec<String>> {
    match outcome.selection() {
        TemporalEvidenceFlowSelection::Selected(value) => Some(
            value
                .identities()
                .get()
                .iter()
                .map(|id| id.get().clone())
                .collect(),
        ),
        _ => None,
    }
}
#[test]
fn all_intents_match_existing_owner_selection_and_preserve_window_identity() {
    let entries = inputs();
    let batch = TemporalEvidenceBatch {
        reference: window(vec![], true).reference().clone(),
        candidates: TemporalEvidenceCandidates::new(
            BoundedSequence::try_from_iter(entries.clone()).unwrap(),
        )
        .unwrap(),
        earliest_history_complete: true,
    };
    let intents = [
        TemporalRetrievalIntent::EarliestEvidence,
        TemporalRetrievalIntent::LatestEvidence,
        TemporalRetrievalIntent::EventOrdering,
        TemporalRetrievalIntent::state_valid_at(15).unwrap(),
        TemporalRetrievalIntent::transition(TransitionDirection::IntoState).unwrap(),
        TemporalRetrievalIntent::duration_since(EntityBoundary::Created).unwrap(),
        TemporalRetrievalIntent::evidence_within(TemporalRetrievalWindow::new(10, 20).unwrap())
            .unwrap(),
    ];
    let native = window(entries, true);
    let encoded = native.clone().encode().unwrap();
    let mut selector = BoundedTemporalEvidenceFlow::prepare();
    for intent in intents {
        let outcome = TemporalEvidenceOutcome::decode(
            selector
                .execute(&encoded, &intent.clone().encode().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(outcome.window_identity(), native.identity());
        match batch.select(&intent).unwrap() {
            TemporalEvidenceSelection::Selected(value) => assert_eq!(
                selected(&outcome).unwrap(),
                value
                    .identities()
                    .get()
                    .iter()
                    .map(|id| id.get().clone())
                    .collect::<Vec<_>>()
            ),
            TemporalEvidenceSelection::NeedEarlierHistory => assert!(matches!(
                outcome.selection(),
                TemporalEvidenceFlowSelection::NeedEarlierHistory
            )),
            TemporalEvidenceSelection::BoundaryUnavailable => assert!(matches!(
                outcome.selection(),
                TemporalEvidenceFlowSelection::BoundaryUnavailable
            )),
        }
    }
}
#[test]
fn empty_incomplete_and_no_matching_boundary_remain_distinct() {
    let mut selector = BoundedTemporalEvidenceFlow::prepare();
    let latest = TemporalRetrievalIntent::LatestEvidence.encode().unwrap();
    let empty = window(vec![], false).encode().unwrap();
    let output =
        TemporalEvidenceOutcome::decode(selector.execute(&empty, &latest).unwrap()).unwrap();
    assert!(matches!(
        output.selection(),
        TemporalEvidenceFlowSelection::NoHistory
    ));
    let early = TemporalRetrievalIntent::EarliestEvidence.encode().unwrap();
    let incomplete = window(inputs(), false).encode().unwrap();
    let output =
        TemporalEvidenceOutcome::decode(selector.execute(&incomplete, &early).unwrap()).unwrap();
    assert!(matches!(
        output.selection(),
        TemporalEvidenceFlowSelection::NeedEarlierHistory
    ));
    let missing = TemporalRetrievalIntent::transition(TransitionDirection::OutOfState)
        .unwrap()
        .encode()
        .unwrap();
    let complete = window(inputs(), true).encode().unwrap();
    let output =
        TemporalEvidenceOutcome::decode(selector.execute(&complete, &missing).unwrap()).unwrap();
    assert!(matches!(
        output.selection(),
        TemporalEvidenceFlowSelection::BoundaryUnavailable
    ));
    let output =
        TemporalEvidenceOutcome::decode(selector.execute(&incomplete, &missing).unwrap()).unwrap();
    assert!(matches!(
        output.selection(),
        TemporalEvidenceFlowSelection::NeedEarlierHistory
    ));
}
#[test]
fn invalid_correlation_duplicate_and_wrong_native_type_refuse_atomically() {
    let mut selector = BoundedTemporalEvidenceFlow::prepare();
    let intent = TemporalRetrievalIntent::LatestEvidence.encode().unwrap();
    let valid = window(inputs(), true).encode().unwrap();
    selector.execute(&valid, &intent).unwrap();
    let one = candidate("same", 10);
    let duplicate = window(vec![one.clone(), one], true).encode().unwrap();
    assert_eq!(
        selector.execute(&duplicate, &intent),
        Err(TemporalEvidenceFlowRefusal::Selection(
            TemporalEvidenceSelectionRefusal::DuplicateIdentity
        ))
    );
    assert!(selector.output().is_empty());
    let mut wrong = candidate("wrong", 10);
    wrong.provenance.reference_at = 101;
    let wrong = window(vec![wrong], true).encode().unwrap();
    assert_eq!(
        selector.execute(&wrong, &intent),
        Err(TemporalEvidenceFlowRefusal::Selection(
            TemporalEvidenceSelectionRefusal::ReferenceMismatch
        ))
    );
    let mut missing = candidate("missing", 10);
    missing.provenance.event_at = None;
    assert_eq!(
        selector.execute(&window(vec![missing], true).encode().unwrap(), &intent),
        Err(TemporalEvidenceFlowRefusal::Selection(
            TemporalEvidenceSelectionRefusal::MissingSourceTime
        ))
    );
    assert_eq!(
        selector.execute(&intent, &valid),
        Err(TemporalEvidenceFlowRefusal::Malformed)
    );
    assert!(selector.output().is_empty());
    let oversized = vec![0; TEMPORAL_EVIDENCE_FLOW_MAX_BYTES + 1];
    assert_eq!(
        selector.execute(&oversized, &intent),
        Err(TemporalEvidenceFlowRefusal::ByteBound)
    );
    assert!(selector.output().is_empty());
    assert!(selector.execute(&valid, &intent).is_ok());
}
#[test]
fn successive_window_intent_changes_and_refusal_execute_without_allocations() {
    let valid = window(inputs(), true).encode().unwrap();
    let empty = window(vec![], true).encode().unwrap();
    let latest = TemporalRetrievalIntent::LatestEvidence.encode().unwrap();
    let earliest = TemporalRetrievalIntent::EarliestEvidence.encode().unwrap();
    let mut selector = BoundedTemporalEvidenceFlow::prepare();
    let ((first, next, none, refused, clear), allocation) = allocation_probe::observe(|| {
        let first = selector.execute(&valid, &latest).is_ok();
        let next = selector.execute(&valid, &earliest).is_ok();
        let none = selector.execute(&empty, &earliest).is_ok();
        let refused =
            selector.execute(b"invalid", &latest) == Err(TemporalEvidenceFlowRefusal::Malformed);
        let clear = selector.output().is_empty();
        (first, next, none, refused, clear)
    });
    assert!(first && next && none && refused && clear);
    assert_eq!(allocation.allocations, 0);
    assert_eq!(allocation.reallocations, 0);
}
#[test]
fn contract_is_ordinary_closing_window_flow_with_current_intent() {
    let contract = temporal_evidence_flow_contract();
    contract.validate().unwrap();
    assert_eq!(contract.kind_id.as_str(), "retrieval/temporal-select-flow");
    assert_eq!(
        contract.inputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
    assert_eq!(
        contract.inputs[1].temporal,
        conduit_core::PortTemporal::Current
    );
    assert_eq!(
        contract.outputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_temporal_evidence_flow_catalog(&mut startup, &mut profile).unwrap();
    assert!(startup.signature(TEMPORAL_EVIDENCE_FLOW_KIND).is_some());
}

#[test]
fn malformed_sequence_validity_and_unit_payload_are_refused_before_selection() {
    use conduit_core::validate_canonical_structured_value;
    let mut selector = BoundedTemporalEvidenceFlow::prepare();
    let valid = window(inputs(), true).encode().unwrap();
    let intent = TemporalRetrievalIntent::EarliestEvidence.encode().unwrap();
    selector.execute(&valid, &intent).unwrap();
    let original = validate_canonical_structured_value(&valid).unwrap();
    let entries = original.record_field("candidates").unwrap().unwrap();
    let offset = entries.value_node().as_ptr() as usize - valid.as_ptr() as usize;
    let mut oversized_count = valid.clone();
    oversized_count[offset + 1..offset + 5].copy_from_slice(&129u32.to_le_bytes());
    assert!(validate_canonical_structured_value(&oversized_count).is_err());
    assert_eq!(
        selector.execute(&oversized_count, &intent),
        Err(TemporalEvidenceFlowRefusal::Malformed)
    );
    assert!(selector.output().is_empty());
    let validity = entries
        .collection_index(0)
        .unwrap()
        .unwrap()
        .record_field("validity")
        .unwrap()
        .unwrap();
    let offset = validity.value_node().as_ptr() as usize - valid.as_ptr() as usize;
    // Every fixture candidate is Historical; replace only the active value tag.
    let mut unknown = valid.clone();
    unknown[offset + 5..offset + 15].copy_from_slice(b"invalidxxx");
    assert!(validate_canonical_structured_value(&unknown).is_err());
    assert_eq!(
        selector.execute(&unknown, &intent),
        Err(TemporalEvidenceFlowRefusal::Malformed)
    );
    let view = validate_canonical_structured_value(&intent).unwrap();
    let unit = view.variant_payload("earliest_evidence").unwrap().unwrap();
    let offset = unit.value_node().as_ptr() as usize - intent.as_ptr() as usize;
    let mut nonempty = intent.clone();
    nonempty[offset + 1..offset + 5].copy_from_slice(&1u32.to_le_bytes());
    nonempty.insert(offset + 5, 0);
    assert!(validate_canonical_structured_value(&nonempty).is_err());
    assert_eq!(
        selector.execute(&valid, &nonempty),
        Err(TemporalEvidenceFlowRefusal::Malformed)
    );
    assert!(selector.output().is_empty());
}
