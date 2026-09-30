use conduit_ai::{
    BatchOrder, ModelComputeOperation, ObjectiveParticipation, PortableComputeClass,
    VectorIndexHealth, VectorIndexMaintenanceKind,
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
    for participation in [
        ObjectiveParticipation::Optimize,
        ObjectiveParticipation::ObserveOnly,
    ] {
        assert_round_trip(participation);
    }
    for order in [BatchOrder::Stable, BatchOrder::Shuffled] {
        assert_round_trip(order);
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
}
