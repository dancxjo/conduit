use conduit_ai::*;
use conduit_core::StateContinuation;

#[path = "common/training_fixture.rs"]
mod fixture;
use fixture::*;

#[test]
fn three_atomic_steps_commit_through_explicit_state_then_checkpoint() {
    let signature = signature();
    let artifact = artifact(&signature);
    let (dataset, split) = corpus();
    let session = session(&artifact, &dataset, &split);
    session.validate(&artifact, &dataset, &split).unwrap();
    assert_ne!(
        session
            .semantic_digest(&artifact, &dataset, &split)
            .unwrap(),
        [0; 32]
    );
    let mut state = TrainingState {
        session_identity: session.identity,
        model: MutableModelState {
            base_artifact_identity: artifact.content_identity(),
            state_identity: "training/tongues/model-state".into(),
            state_schema_version: 1,
            generation: 0,
        },
        initial_generation: 0,
        completed_steps: 0,
        consumed_work_units: 0,
    };
    let boundary = session
        .planned_state_boundary(&artifact, &state, 64)
        .unwrap();
    assert_eq!(
        boundary.continuation,
        StateContinuation::MaximumTransitions(3)
    );

    let realization = realization();
    realization
        .as_model_runtime(&artifact)
        .admit(&artifact)
        .unwrap();
    for step in 1_u8..=3 {
        let request = TrainStepRequest {
            step: u64::from(step),
            expected_generation: state.model.generation,
            batch: batch(
                step - 1,
                if step == 2 {
                    &["audio"]
                } else {
                    &["audio", "ema"]
                },
            ),
            admitted_work_units: 1000,
        };
        let candidate = HostStepCandidate {
            state_identity: state.model.state_identity.clone(),
            state_schema_version: state.model.state_schema_version,
            generation: state.model.generation + 1,
            metrics: metrics(step),
            consumed_work_units: 900,
        };
        let outcome = session
            .commit_step(TrainStepCommit {
                artifact: &artifact,
                dataset: &dataset,
                split: &split,
                state: &state,
                request: &request,
                terminal: HostStepTerminal::Candidate(candidate),
                realization: &realization,
            })
            .unwrap();
        let TrainStepOutcome::Committed(committed) = outcome else {
            panic!("successful Host candidate must commit")
        };
        let CommittedTrainingStep {
            state: next,
            receipt,
        } = *committed;
        assert_eq!(receipt.prior_generation + 1, receipt.generation);
        assert_ne!(receipt.semantic_digest(), [0; 32]);
        state = next;
    }
    assert_eq!(state.model.generation, 3);
    assert_eq!(state.consumed_work_units, 2700);
    let checkpoint = ModelCheckpoint {
        base_artifact_identity: artifact.content_identity(),
        architecture_profile: artifact.architecture_profile.clone(),
        state_schema_version: state.model.state_schema_version,
        generation: state.model.generation,
        content: resource(30, MODEL_CHECKPOINT_INFO_ID, 8192),
    };
    let receipt = session
        .checkpoint(CheckpointRequest {
            artifact: &artifact,
            dataset: &dataset,
            split: &split,
            state: &state,
            checkpoint,
            metric_summaries: metrics(3),
            realization: &realization,
        })
        .unwrap();
    assert_eq!(receipt.completed_steps, 3);
    assert_eq!(receipt.consumed_work_units, 2700);
    assert_eq!(receipt.checkpoint.generation, 3);
    assert_eq!(
        receipt.session_descriptor_identity,
        session
            .semantic_digest(&artifact, &dataset, &split)
            .unwrap()
    );
    assert_ne!(receipt.semantic_digest(), [0; 32]);
}

#[test]
fn cancellation_failure_and_evaluation_never_commit_state() {
    let signature = signature();
    let artifact = artifact(&signature);
    let (dataset, split) = corpus();
    let session = session(&artifact, &dataset, &split);
    let state = TrainingState {
        session_identity: session.identity,
        model: MutableModelState {
            base_artifact_identity: artifact.content_identity(),
            state_identity: "training/tongues/model-state".into(),
            state_schema_version: 1,
            generation: 1,
        },
        initial_generation: 0,
        completed_steps: 1,
        consumed_work_units: 900,
    };
    let request = TrainStepRequest {
        step: 2,
        expected_generation: 1,
        batch: batch(0, &["audio", "ema"]),
        admitted_work_units: 1000,
    };
    for failure in [
        TrainStepFailure::Cancelled,
        TrainStepFailure::ResourceExhausted,
        TrainStepFailure::ProviderLost,
        TrainStepFailure::Failed,
    ] {
        assert_eq!(
            session
                .commit_step(TrainStepCommit {
                    artifact: &artifact,
                    dataset: &dataset,
                    split: &split,
                    state: &state,
                    request: &request,
                    terminal: HostStepTerminal::NoCommit(failure),
                    realization: &realization(),
                })
                .unwrap(),
            TrainStepOutcome::NotCommitted {
                failure,
                retained_generation: state.model.generation,
            }
        );
    }
    let evaluation = session
        .evaluate(EvaluationRequest {
            artifact: &artifact,
            dataset: &dataset,
            split: &split,
            state: &state,
            batch: &request.batch,
            metrics: metrics(2),
            consumed_work_units: 500,
            realization: &realization(),
        })
        .unwrap();
    assert_eq!(evaluation.state_generation, state.model.generation);
    assert_eq!(evaluation.state_identity, state.model.state_identity);
    assert_ne!(evaluation.semantic_digest(), [0; 32]);
    let mut disabled = session.clone();
    disabled.evaluation_policy = EvaluationPolicy::None;
    assert_eq!(
        disabled.evaluate(EvaluationRequest {
            artifact: &artifact,
            dataset: &dataset,
            split: &split,
            state: &state,
            batch: &request.batch,
            metrics: metrics(2),
            consumed_work_units: 500,
            realization: &realization(),
        }),
        Err(TrainingRefusal::EvaluationNotScheduled)
    );
}

#[test]
fn stale_half_steps_bad_splits_and_resource_overruns_refuse() {
    let signature = signature();
    let artifact = artifact(&signature);
    let (dataset, split) = corpus();
    let mut session = session(&artifact, &dataset, &split);
    let state = TrainingState {
        session_identity: session.identity,
        model: MutableModelState {
            base_artifact_identity: artifact.content_identity(),
            state_identity: "training/tongues/model-state".into(),
            state_schema_version: 1,
            generation: 1,
        },
        initial_generation: 0,
        completed_steps: 1,
        consumed_work_units: 900,
    };
    let mut request = TrainStepRequest {
        step: 2,
        expected_generation: 0,
        batch: batch(0, &["audio", "ema"]),
        admitted_work_units: 1000,
    };
    let candidate = HostStepCandidate {
        state_identity: state.model.state_identity.clone(),
        state_schema_version: 1,
        generation: 2,
        metrics: metrics(2),
        consumed_work_units: 900,
    };
    assert_eq!(
        session.commit_step(TrainStepCommit {
            artifact: &artifact,
            dataset: &dataset,
            split: &split,
            state: &state,
            request: &request,
            terminal: HostStepTerminal::Candidate(candidate.clone()),
            realization: &realization(),
        }),
        Err(TrainingRefusal::StaleState)
    );
    request.expected_generation = 1;
    request.batch.example_identities = vec![[99; 32]];
    assert_eq!(
        session.commit_step(TrainStepCommit {
            artifact: &artifact,
            dataset: &dataset,
            split: &split,
            state: &state,
            request: &request,
            terminal: HostStepTerminal::Candidate(candidate.clone()),
            realization: &realization(),
        }),
        Err(TrainingRefusal::InvalidBatch)
    );
    request.batch = batch(0, &["audio", "ema"]);
    request.admitted_work_units = 800;
    assert_eq!(
        session.commit_step(TrainStepCommit {
            artifact: &artifact,
            dataset: &dataset,
            split: &split,
            state: &state,
            request: &request,
            terminal: HostStepTerminal::Candidate(candidate),
            realization: &realization(),
        }),
        Err(TrainingRefusal::WorkBoundExceeded)
    );
    request.admitted_work_units = 1000;
    let mut nearly_exhausted = state.clone();
    nearly_exhausted.consumed_work_units = 9500;
    let aggregate_candidate = HostStepCandidate {
        state_identity: state.model.state_identity.clone(),
        state_schema_version: 1,
        generation: 2,
        metrics: metrics(2),
        consumed_work_units: 900,
    };
    assert_eq!(
        session.commit_step(TrainStepCommit {
            artifact: &artifact,
            dataset: &dataset,
            split: &split,
            state: &nearly_exhausted,
            request: &request,
            terminal: HostStepTerminal::Candidate(aggregate_candidate),
            realization: &realization(),
        }),
        Err(TrainingRefusal::WorkBoundExceeded)
    );
    let early_checkpoint = ModelCheckpoint {
        base_artifact_identity: artifact.content_identity(),
        architecture_profile: artifact.architecture_profile.clone(),
        state_schema_version: 1,
        generation: state.model.generation,
        content: resource(31, MODEL_CHECKPOINT_INFO_ID, 8192),
    };
    assert_eq!(
        session.checkpoint(CheckpointRequest {
            artifact: &artifact,
            dataset: &dataset,
            split: &split,
            state: &state,
            checkpoint: early_checkpoint,
            metric_summaries: metrics(1),
            realization: &realization(),
        }),
        Err(TrainingRefusal::CheckpointNotScheduled)
    );
    session.resources.maximum_in_flight_steps = 2;
    assert_eq!(
        session.validate(&artifact, &dataset, &split),
        Err(TrainingRefusal::InvalidResourceEnvelope)
    );
}
