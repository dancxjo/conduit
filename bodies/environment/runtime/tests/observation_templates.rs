//! Independent ordinary constructors are the byte oracle for linked templates.
use conduit_core::{
    self as c, StructuredFieldValue as Field, StructuredInfoType as Type,
    StructuredInfoTypeShape as Shape, StructuredInfoValue as Value,
};
use conduit_environment_runtime::observation::{
    ObservationContract, PreparedObservationCodec, PreparedObservationQuantityForms,
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn variant<'a>(ty: &'a Type, tag: &str) -> &'a Type {
    let Shape::Variant { cases, .. } = ty.shape() else {
        panic!()
    };
    cases
        .iter()
        .find(|c| c.tag() == tag)
        .unwrap()
        .payload_type()
}
fn field<'a>(ty: &'a Type, name: &str) -> &'a Type {
    let Shape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
#[test]
fn one_output_buffer_matches_all_quantity_and_failure_envelopes_without_play_allocation() {
    let contract = ObservationContract::prepare().unwrap();
    let (mut codec, memory) =
        allocation_probe::observe(|| PreparedObservationCodec::prepare(&contract).unwrap());
    let maximum = conduit_plot::maximum_prepared_canonical_value_bytes(contract.result_type())
        .unwrap() as usize;
    assert_eq!(memory.retained_bytes(), maximum);
    let (forms, memory) =
        allocation_probe::observe(|| PreparedObservationQuantityForms::prepare(&contract).unwrap());
    assert_eq!(
        memory.retained_bytes(),
        conduit_plot::maximum_prepared_canonical_value_bytes(contract.observation_type()).unwrap()
            as usize
    );
    drop(forms);
    let mut cases = Vec::new();
    for (temperature, humidity) in [(-108i128, 530i128), (0, 0), (198, 590), (600, 950)] {
        let frame_type = variant(contract.input_type(), "frame");
        let frame = Value::record(
            frame_type.clone(),
            vec![
                Field::new(
                    "temperature",
                    Value::leaf(
                        field(frame_type, "temperature").clone(),
                        temperature.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap(),
                Field::new(
                    "humidity",
                    Value::leaf(
                        field(frame_type, "humidity").clone(),
                        humidity.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let input = Value::variant(contract.input_type().clone(), "frame", frame)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let observation_type = contract.observation_type();
        let observation = Value::record(
            observation_type.clone(),
            vec![
                Field::new(
                    "temperature",
                    Value::leaf(
                        field(observation_type, "temperature").clone(),
                        c::Quantity::from_decimal(temperature, -1, c::Unit::Celsius)
                            .unwrap()
                            .encode()
                            .to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap(),
                Field::new(
                    "humidity",
                    Value::leaf(
                        field(observation_type, "humidity").clone(),
                        c::Quantity::from_decimal(humidity, -1, c::Unit::Percent)
                            .unwrap()
                            .encode()
                            .to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let expected = Value::variant(contract.result_type().clone(), "observation", observation)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        cases.push((input, expected));
    }
    let failure_type = variant(contract.input_type(), "unavailable");
    for (tag, payload) in [
        ("short-frame", vec![3]),
        ("malformed-length", vec![6]),
        ("checksum", vec![]),
        ("invalid-payload", vec![]),
    ] {
        let failure = Value::variant(
            failure_type.clone(),
            tag,
            Value::leaf(variant(failure_type, tag).clone(), payload).unwrap(),
        )
        .unwrap();
        let input = Value::variant(
            contract.input_type().clone(),
            "unavailable",
            failure.clone(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let expected = Value::variant(contract.result_type().clone(), "unavailable", failure)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        cases.push((input, expected));
    }
    let ((), memory) = allocation_probe::observe(|| {
        for _ in 0..8 {
            for (input, expected) in &cases {
                assert_eq!(codec.convert(input).unwrap(), expected);
            }
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
