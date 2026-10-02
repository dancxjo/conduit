use conduit_ai::{
    BatchOrder, CheckpointPolicy, EvaluationPolicy, ModelComputeLifecycle, ModelComputeOperation,
    ModelComputeRefusal, ModelSignatureRefusal, ObjectiveParticipation, PortableComputeClass,
    RelationQueryMode, RelationRefusal, TrainingObjective, TrainingObjectiveIdentity,
    TrainingRefusal, TrainingResourceEnvelope, VectorIndexHealth, VectorIndexMaintenanceKind,
    VectorIndexResourceRefusal,
};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn training_vocabularies_round_trip_through_native_types() {
    let objective_identity = |value: &str| TrainingObjectiveIdentity::new(value.into()).unwrap();
    let objective = TrainingObjective::new(
        objective_identity("acoustic-reconstruction"),
        1_000_000,
        objective_identity("tongues/acoustic-reconstruction@1"),
        objective_identity("loss/acoustic"),
        ObjectiveParticipation::Optimize,
    )
    .unwrap();
    let structured = objective.clone().into_structured().unwrap();
    assert_eq!(
        TrainingObjective::from_structured(structured).unwrap(),
        objective
    );
    assert!(TrainingObjective::new(
        objective_identity("invalid-zero-weight"),
        0,
        objective_identity("tongues/invalid-zero-weight@1"),
        objective_identity("loss/invalid"),
        ObjectiveParticipation::Optimize,
    )
    .is_err());
    assert!(
        !include_str!("../src/training.rs").contains(concat!("pub struct ", "TrainingObjective"))
    );

    assert_round_trip(
        TrainingResourceEnvelope::new(4096, 1_048_576, 2, 4096, 65_536, 3, 10_000, 16_384, 1)
            .unwrap(),
    );
    assert!(
        TrainingResourceEnvelope::new(4096, 1_048_576, 2, 4097, 65_536, 3, 10_000, 16_384, 1)
            .is_err()
    );
    assert!(!include_str!("../src/training.rs")
        .contains(concat!("pub struct ", "TrainingResourceEnvelope")));

    for participation in [
        ObjectiveParticipation::Optimize,
        ObjectiveParticipation::ObserveOnly,
    ] {
        assert_round_trip(participation);
    }
    for order in [BatchOrder::Stable, BatchOrder::Shuffled] {
        assert_round_trip(order);
    }
    for policy in [
        CheckpointPolicy::None,
        CheckpointPolicy::EverySteps(7),
        CheckpointPolicy::AtCompletion,
    ] {
        assert_round_trip(policy);
    }
    for policy in [
        EvaluationPolicy::None,
        EvaluationPolicy::EverySteps(11),
        EvaluationPolicy::AtCompletion,
    ] {
        assert_round_trip(policy);
    }
    for operation in [
        ModelComputeOperation::Inference,
        ModelComputeOperation::Encode,
        ModelComputeOperation::Decode,
        ModelComputeOperation::Sample,
        ModelComputeOperation::Score,
        ModelComputeOperation::TrainStep,
        ModelComputeOperation::Evaluate,
        ModelComputeOperation::Checkpoint,
        ModelComputeOperation::IntegrateDynamics,
        ModelComputeOperation::RelationQuery,
    ] {
        assert_round_trip(operation);
    }
    for lifecycle in [
        ModelComputeLifecycle::Discovered,
        ModelComputeLifecycle::Active(ModelComputeOperation::Inference),
        ModelComputeLifecycle::Lost,
        ModelComputeLifecycle::Shutdown,
    ] {
        assert_round_trip(lifecycle);
    }
    for class in [
        PortableComputeClass::GeneralCpu,
        PortableComputeClass::VectorCompute,
        PortableComputeClass::Accelerator,
    ] {
        assert_round_trip(class);
    }
    for health in [VectorIndexHealth::Ready, VectorIndexHealth::Unavailable] {
        assert_round_trip(health);
    }
    for kind in [
        VectorIndexMaintenanceKind::Rebuild,
        VectorIndexMaintenanceKind::Compaction,
    ] {
        assert_round_trip(kind);
    }
    for refusal in [
        ModelComputeRefusal::InvalidOffer,
        ModelComputeRefusal::UnsupportedOperation,
        ModelComputeRefusal::ResourceBoundExceeded,
        ModelComputeRefusal::ProviderUnavailable,
    ] {
        assert_round_trip(refusal);
    }
    for refusal in [
        ModelSignatureRefusal::InvalidIdentity,
        ModelSignatureRefusal::DuplicateOperation,
        ModelSignatureRefusal::InvalidSignalConstraint,
    ] {
        assert_round_trip(refusal);
    }
    for mode in [
        RelationQueryMode::InferPosterior,
        RelationQueryMode::SampleConditional,
        RelationQueryMode::Reconstruct,
        RelationQueryMode::EncodeLatent,
        RelationQueryMode::DecodeGenerate,
        RelationQueryMode::LogProbability,
    ] {
        assert_round_trip(mode);
    }
    for refusal in [
        RelationRefusal::MissingIdentity,
        RelationRefusal::ShapeMismatch,
        RelationRefusal::InvalidRealization,
    ] {
        assert_round_trip(refusal);
    }
    for refusal in [
        TrainingRefusal::InvalidIdentity,
        TrainingRefusal::WorkBoundExceeded,
        TrainingRefusal::InvalidLifecycleTransition,
    ] {
        assert_round_trip(refusal);
    }
    for refusal in [
        VectorIndexResourceRefusal::InvalidIdentity,
        VectorIndexResourceRefusal::ResourceBusy,
        VectorIndexResourceRefusal::InvalidResourceBinding,
    ] {
        assert_round_trip(refusal);
    }
}
