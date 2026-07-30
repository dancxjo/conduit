use conduit_core::{
    ExactProviderBinding, Id, PinnedDescriptor, ProviderBounds, ProviderObservationState,
    SemanticHash,
};
use conduit_runtime::{
    BoundedProviderRun, ProviderRunError, ProviderRunEvidenceKind, ProviderRunPhase,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

fn binding(
    maximum_foreign_queue: u16,
    maximum_cancellation_ticks: u64,
) -> ExactProviderBinding<'static> {
    ExactProviderBinding {
        profile: pin("acme/profile/linux", 1),
        required_contract: pin("acme/contract/weather", 2),
        provider_bundle: pin("acme/provider/weather", 3),
        implementation: pin("acme/implementation/weather", 4),
        artifact: pin("acme/artifact/weather", 5),
        adapter: pin("acme/adapter/native", 6),
        host_report: pin("acme/host-report/linux", 7),
        observation: hash(8),
        satisfaction_proof: hash(9),
        conformance_result: hash(10),
        bounds: ProviderBounds {
            maximum_in_flight: 1,
            maximum_foreign_queue,
            maximum_memory_bytes: 4096,
            maximum_cancellation_ticks,
            maximum_evidence_events: 8,
        },
    }
}

#[test]
fn exact_provider_chain_survives_execution_and_terminal_evidence() {
    let exact = binding(0, 5);
    let mut run = BoundedProviderRun::new(Id("run/weather/1"), exact);
    let started = run.start(10).unwrap();
    assert_eq!(started.kind, ProviderRunEvidenceKind::Started);
    assert_eq!(started.provider_bundle, exact.provider_bundle.semantic_hash);
    assert_eq!(started.observation, exact.observation);
    assert_eq!(started.conformance_result, exact.conformance_result);
    let completed = run.complete(12).unwrap();
    assert_eq!(completed.kind, ProviderRunEvidenceKind::Completed);
    assert_eq!(run.phase(), ProviderRunPhase::Completed);
}

#[test]
fn hidden_foreign_queue_and_non_cancellable_worker_fail_boundedly() {
    let mut queued = BoundedProviderRun::new(Id("run/weather/queue"), binding(0, 5));
    queued.start(10).unwrap();
    assert_eq!(
        queued.set_foreign_queue(1),
        Err(ProviderRunError::ForeignQueueExceeded)
    );
    assert_eq!(queued.phase(), ProviderRunPhase::Terminal);

    let mut cancelling = BoundedProviderRun::new(Id("run/weather/cancel"), binding(0, 2));
    cancelling.start(10).unwrap();
    cancelling.cancel(11).unwrap();
    assert_eq!(
        cancelling.observe_cancelled(14),
        Err(ProviderRunError::CancellationDeadlineExceeded)
    );
}

#[test]
fn provider_loss_is_terminal_without_changing_contract_identity() {
    let exact = binding(0, 5);
    let contract = exact.required_contract;
    let mut run = BoundedProviderRun::new(Id("run/weather/loss"), exact);
    run.start(10).unwrap();
    assert_eq!(
        run.observe_provider_state(ProviderObservationState::Lost, 11),
        Err(ProviderRunError::ProviderLost)
    );
    assert_eq!(run.phase(), ProviderRunPhase::Terminal);
    assert_eq!(exact.required_contract, contract);
}
