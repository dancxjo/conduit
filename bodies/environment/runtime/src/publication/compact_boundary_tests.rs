//! Independent canonical constructors are the oracle for the lossless boundary.
use super::*;
use crate::observation::{
    ObservationContract, PreparedObservationCodec, PreparedObservationEmitter,
};
use crate::publication::source_reading::PackedReading;
fn variant<'a>(ty: &'a Type, tag: &str) -> &'a Type {
    let Shape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|c| c.tag() == tag)
        .unwrap()
        .payload_type()
}
fn field<'a>(ty: &'a Type, name: &str) -> &'a Type {
    let Shape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
#[test]
fn selected_wire_reconstructs_exact_quantities_and_every_failure_without_allocation() {
    let contract = ObservationContract::prepare().unwrap();
    let mut emitter = PreparedObservationEmitter::prepare(&contract).unwrap();
    let mut canonical = PreparedObservationCodec::prepare(&contract).unwrap();
    let mut inputs = Vec::new();
    for (temperature, humidity) in [(-108i128, 530i128), (-1, 1), (0, 0), (198, 590), (600, 950)] {
        let frame = variant(contract.input_type(), "frame");
        let value = Value::record(
            frame.clone(),
            vec![
                c::StructuredFieldValue::new(
                    "temperature",
                    Value::leaf(
                        field(frame, "temperature").clone(),
                        temperature.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap(),
                c::StructuredFieldValue::new(
                    "humidity",
                    Value::leaf(
                        field(frame, "humidity").clone(),
                        humidity.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        inputs.push(
            Value::variant(contract.input_type().clone(), "frame", value)
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
    }
    let failure = variant(contract.input_type(), "unavailable");
    for (tag, payload) in [
        ("short-frame", vec![3]),
        ("malformed-length", vec![6]),
        ("checksum", vec![]),
        ("invalid-payload", vec![]),
    ] {
        let value = Value::variant(
            failure.clone(),
            tag,
            Value::leaf(variant(failure, tag).clone(), payload).unwrap(),
        )
        .unwrap();
        inputs.push(
            Value::variant(contract.input_type().clone(), "unavailable", value)
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
    }
    let mut output = [0; material::RESULT_MAXIMUM];
    let (_, allocation) = allocation_probe::observe(|| {
        for input in &inputs {
            let expected = canonical.convert(input).unwrap();
            let length = emitter.stage(input).unwrap();
            let wire = emitter.compact_output().unwrap();
            assert!(wire.len() <= crate::publication::source_reading::COMPACT_READING_BYTES);
            let packed = PackedReading::from_compact(wire).unwrap();
            assert_eq!(packed.canonical_length(), length);
            assert_eq!(packed.encode(&mut output).unwrap(), length);
            assert_eq!(&output[..length], expected);
            // The generic canonical consumer remains an independent byte oracle.
            let mut direct = [0; material::RESULT_MAXIMUM];
            assert_eq!(emitter.encode(&mut direct).unwrap(), length);
            assert_eq!(&direct[..length], expected);
            emitter.clear();
        }
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
}
#[test]
fn malformed_wire_never_becomes_a_canonical_reading() {
    for bytes in [
        &[][..],
        &[2][..],
        &[0][..],
        &[0, 0, 0, 0][..],
        &[1, 255][..],
        &[1; 29][..],
    ] {
        assert!(PackedReading::from_compact(bytes).is_err());
    }
}
