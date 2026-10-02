use conduit_data::{
    DataGenerationNamespaceRefusal, DataGenerationRefusal, DataReferenceRefusal,
    ScientificAlignmentRefusal, ScientificCorpusRefusal, ScientificObservationRefusal,
    TensorRefusal,
};
use conduit_form::rust_binding::NativeRustBinding;

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn corpus_refusal_keeps_nested_observation_meaning() {
    round_trip(
        ScientificCorpusRefusal::observation(ScientificObservationRefusal::MissingIdentity)
            .unwrap(),
    );
    round_trip(ScientificCorpusRefusal::SplitLeakage);
    round_trip(
        ScientificAlignmentRefusal::observation(ScientificObservationRefusal::ClockMismatch)
            .unwrap(),
    );
    round_trip(ScientificAlignmentRefusal::CalibrationShapeMismatch);
}

#[test]
fn tensor_and_observation_refusals_are_native() {
    for value in [
        TensorRefusal::RankOutOfBounds,
        TensorRefusal::ZeroDimension,
        TensorRefusal::AxisCountMismatch,
        TensorRefusal::AxisIdentityInvalid,
        TensorRefusal::ShapeOverflow,
        TensorRefusal::ByteBoundExceeded,
        TensorRefusal::InlinePayloadTooLarge,
        TensorRefusal::PayloadLengthMismatch,
        TensorRefusal::ResourceProfileMismatch,
        TensorRefusal::ResourceExtentMismatch,
        TensorRefusal::ContentIdentityMismatch,
        TensorRefusal::InvalidResource,
        TensorRefusal::UnsupportedEncodingVersion,
        TensorRefusal::UnsupportedElement,
        TensorRefusal::UnsupportedAxisRole,
        TensorRefusal::UnsupportedUnit,
        TensorRefusal::MalformedEncoding,
    ] {
        round_trip(value);
    }
    for value in [
        ScientificObservationRefusal::MissingIdentity,
        ScientificObservationRefusal::InvalidIdentity,
        ScientificObservationRefusal::InvalidValue,
        ScientificObservationRefusal::ClockMismatch,
        ScientificObservationRefusal::MissingSource,
        ScientificObservationRefusal::TooManySources,
        ScientificObservationRefusal::EmptyObservationSet,
        ScientificObservationRefusal::TooManyObservations,
        ScientificObservationRefusal::DuplicateObservation,
        ScientificObservationRefusal::InvalidMask,
        ScientificObservationRefusal::MaskShapeMismatch,
        ScientificObservationRefusal::UnknownMaskedObservation,
        ScientificObservationRefusal::DuplicateMask,
    ] {
        round_trip(value);
    }
}

#[test]
fn immutable_data_reference_refusals_are_native() {
    for value in [
        DataReferenceRefusal::Malformed,
        DataReferenceRefusal::WrongReferenceKind,
        DataReferenceRefusal::WrongContentKind,
        DataReferenceRefusal::WrongAccessClass,
        DataReferenceRefusal::ExpiringGeneration,
        DataReferenceRefusal::ItemExtent,
    ] {
        round_trip(value);
    }
    for value in [
        DataGenerationNamespaceRefusal::Empty,
        DataGenerationNamespaceRefusal::TooLarge,
    ] {
        round_trip(value);
    }

    for value in [
        DataGenerationRefusal::InvalidBounds,
        DataGenerationRefusal::WrongContentKind,
        DataGenerationRefusal::ValueTooLarge,
        DataGenerationRefusal::GenerationCapacityExhausted,
        DataGenerationRefusal::ByteCapacityExhausted,
        DataGenerationRefusal::ReferenceOutputCapacity,
        DataGenerationRefusal::GenerationNotRetained,
        DataGenerationRefusal::ExtentMismatch,
    ] {
        round_trip(value);
    }
    round_trip(DataGenerationRefusal::reference(DataReferenceRefusal::WrongAccessClass).unwrap());
}
