//! Checked finite feedback graph; hosted Plan/Play is a separate proof surface.
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
#[path = "common/resonator.rs"]
mod fixture;
use fixture::{field, integer, leaf};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
struct Chain {
    programs: Vec<PortableExpressionProgram>,
    prepared: Vec<PreparedPortableExpressionEvaluator>,
}
impl Chain {
    fn new(entry: &str) -> Self {
        Self::from_programs(fixture::programs(entry))
    }
    fn from_programs(programs: Vec<PortableExpressionProgram>) -> Self {
        let prepared = programs
            .iter()
            .map(|p| PreparedPortableExpressionEvaluator::new(p).unwrap())
            .collect();
        Self { programs, prepared }
    }
    fn input_type(&self) -> &StructuredInfoType {
        &self.programs[0].input_type
    }
    fn run(&mut self, input: &StructuredInfoValue) -> StructuredInfoValue {
        let mut bytes = input.canonical_bytes().unwrap();
        for evaluator in &mut self.prepared {
            let capacity = evaluator.output_capacity();
            let (output, allocation) =
                allocation_probe::observe(|| evaluator.evaluate(&bytes).unwrap());
            assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
            bytes = output.to_vec();
            assert_eq!(evaluator.output_capacity(), capacity);
        }
        StructuredInfoValue::from_canonical_bytes(&bytes).unwrap()
    }
}
fn record(ty: &StructuredInfoType, values: &[(&str, StructuredInfoValue)]) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|f| {
                StructuredFieldValue::new(
                    f.name(),
                    values
                        .iter()
                        .find(|(n, _)| *n == f.name())
                        .unwrap()
                        .1
                        .clone(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
fn admitted_request(ty: &StructuredInfoType, frames: u32) -> StructuredInfoValue {
    let original = fixture::request(ty, "", (1000, 1), (100, 1), 8000);
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("request")
    };
    let values = fields
        .iter()
        .map(|f| {
            (
                f.name(),
                if f.name() == "frames" {
                    StructuredInfoValue::leaf(f.value_type().clone(), frames.to_le_bytes().to_vec())
                        .unwrap()
                } else {
                    field(&original, f.name()).clone()
                },
            )
        })
        .collect::<Vec<_>>();
    record(ty, &values)
}
fn outcome(value: &StructuredInfoValue, expected: &str) -> StructuredInfoValue {
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("outcome")
    };
    assert_eq!(tag, expected);
    payload.clone()
}
fn step_input(
    ty: &StructuredInfoType,
    cursor: &StructuredInfoValue,
    sample: i128,
    cancelled: bool,
) -> StructuredInfoValue {
    record(
        ty,
        &[
            ("cursor", cursor.clone()),
            (
                "sample",
                StructuredInfoValue::leaf(
                    field_type(ty, "sample").clone(),
                    sample.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
            (
                "cancelled",
                StructuredInfoValue::leaf(
                    field_type(ty, "cancelled").clone(),
                    vec![u8::from(cancelled)],
                )
                .unwrap(),
            ),
        ],
    )
}
fn altered_cursor(
    ty: &StructuredInfoType,
    cursor: &StructuredInfoValue,
    name: &str,
    bytes: Vec<u8>,
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    let values = fields
        .iter()
        .map(|f| {
            (
                f.name(),
                if f.name() == name {
                    StructuredInfoValue::leaf(f.value_type().clone(), bytes.clone()).unwrap()
                } else {
                    field(cursor, f.name()).clone()
                },
            )
        })
        .collect::<Vec<_>>();
    record(ty, &values)
}
#[test]
fn actual_impulse_state_matches_independent_q14_recurrence_and_frequency_response() {
    let mut preparation = Chain::new("dsp/resonator-prepare");
    assert_eq!(preparation.programs.len(), 24);
    let request = admitted_request(preparation.input_type(), 512);
    let start = preparation.run(&request);
    let initial = outcome(&start, "started");
    assert_eq!(outcome(&preparation.run(&request), "started"), initial);
    let mut step = Chain::new("dsp/resonator-step");
    assert_eq!(step.programs.len(), 27);
    let mut resumed = Chain::from_programs(step.programs.clone());
    let mut cursor = initial.clone();
    let mut resumed_cursor = initial;
    let (mut y1, mut y2) = (0i128, 0i128);
    let mut impulse = Vec::new();
    // Independent Q14 coefficients rounded from exp/cos at 1000Hz/100Hz/8kHz.
    let (b, c) = (22278i128, -15146i128);
    for index in 0..512u32 {
        let sample = if index == 0 { 32767 } else { 0 };
        let expected = (sample * 16384 + b * y1 + c * y2) / 16384;
        let input = step_input(step.input_type(), &cursor, sample, false);
        let advanced = outcome(&step.run(&input), "advanced");
        assert_eq!(integer(&advanced, "sample"), expected);
        cursor = field(&advanced, "cursor").clone();
        assert_eq!(integer(&cursor, "y1"), expected);
        assert_eq!(integer(&cursor, "y2"), y1);
        assert_eq!(field(&cursor, "request"), &request);
        assert_eq!(
            u32::from_le_bytes(leaf(field(&cursor, "remaining")).try_into().unwrap()),
            511 - index
        );
        if index == 127 || index == 319 {
            resumed = Chain::from_programs(step.programs.clone());
            resumed_cursor = StructuredInfoValue::from_canonical_bytes(
                &resumed_cursor.canonical_bytes().unwrap(),
            )
            .unwrap();
        }
        let resumed_input = step_input(resumed.input_type(), &resumed_cursor, sample, false);
        let resumed_advanced = outcome(&resumed.run(&resumed_input), "advanced");
        resumed_cursor = field(&resumed_advanced, "cursor").clone();
        assert_eq!(resumed_cursor, cursor);
        y2 = y1;
        y1 = expected;
        impulse.push(expected as f64 / 32767.0);
    }
    let exhausted_input = step_input(step.input_type(), &cursor, 0, false);
    assert_eq!(outcome(&step.run(&exhausted_input), "exhausted"), cursor);
    for frequency in [0.0, 500.0, 1000.0, 1500.0, 3000.0, 3999.0] {
        let omega = 2.0 * std::f64::consts::PI * frequency / 8000.0;
        let measured_re = impulse
            .iter()
            .enumerate()
            .map(|(n, y)| y * (omega * n as f64).cos())
            .sum::<f64>();
        let measured_im = -impulse
            .iter()
            .enumerate()
            .map(|(n, y)| y * (omega * n as f64).sin())
            .sum::<f64>();
        let dr =
            1.0 - (b as f64 / 16384.0) * omega.cos() - (c as f64 / 16384.0) * (2.0 * omega).cos();
        let di = (b as f64 / 16384.0) * omega.sin() + (c as f64 / 16384.0) * (2.0 * omega).sin();
        let norm = dr * dr + di * di;
        let expected_re = dr / norm;
        let expected_im = -di / norm;
        let tolerance = 0.002 + 0.005 / norm.sqrt();
        assert!(
            (measured_re - expected_re).hypot(measured_im - expected_im) <= tolerance,
            "frequency={frequency}"
        );
    }
}
#[test]
fn forged_extremes_budget_cancellation_and_overflow_have_distinct_safe_outcomes() {
    let mut prep = Chain::new("dsp/resonator-prepare");
    let request = admitted_request(prep.input_type(), 64);
    let initial = outcome(&prep.run(&request), "started");
    let mut step = Chain::new("dsp/resonator-step");
    let input = step_input(step.input_type(), &initial, 0, true);
    assert_eq!(outcome(&step.run(&input), "cancelled"), initial);
    for sample in [i128::MIN, i128::MAX, -32769, 32768] {
        let input = step_input(step.input_type(), &initial, sample, false);
        outcome(&step.run(&input), "invalid_cursor");
    }
    for (name, bytes) in [
        ("y1", i128::MIN.to_le_bytes().to_vec()),
        ("y2", i128::MAX.to_le_bytes().to_vec()),
        ("remaining", 65u32.to_le_bytes().to_vec()),
        ("emitted", u32::MAX.to_le_bytes().to_vec()),
    ] {
        let cursor = altered_cursor(
            field_type(step.input_type(), "cursor"),
            &initial,
            name,
            bytes,
        );
        let input = step_input(step.input_type(), &cursor, 0, false);
        outcome(&step.run(&input), "invalid_cursor");
    }
    let ty = field_type(step.input_type(), "cursor");
    let cursor = altered_cursor(
        ty,
        &initial,
        "y1",
        i128::from(i32::MAX).to_le_bytes().to_vec(),
    );
    let cursor = altered_cursor(
        ty,
        &cursor,
        "y2",
        i128::from(i32::MIN).to_le_bytes().to_vec(),
    );
    let input = step_input(step.input_type(), &cursor, 0, false);
    outcome(&step.run(&input), "overflow");
}

#[test]
fn pole_admission_and_last_legal_budget_are_explicit() {
    let mut project = Chain::new("dsp/resonator-project");
    let request = admitted_request(project.input_type(), 65536);
    let projection = project.run(&request);
    let mut admission = Chain::new("dsp/resonator-start-value");
    for (b, c) in [
        (0, 0),
        (0, -16384),
        (31530, -15146),
        (-31530, -15146),
        (i128::MIN, -15146),
        (0, i128::MIN),
        (0, i128::MAX),
    ] {
        let candidate = altered_cursor(
            admission.input_type(),
            &projection,
            "b_q14",
            b.to_le_bytes().to_vec(),
        );
        let candidate = altered_cursor(
            admission.input_type(),
            &candidate,
            "c_q14",
            c.to_le_bytes().to_vec(),
        );
        outcome(&admission.run(&candidate), "unstable_projection");
    }
    let initial = outcome(&admission.run(&projection), "started");
    let mut step = Chain::new("dsp/resonator-step");
    let ty = field_type(step.input_type(), "cursor");
    let last = altered_cursor(ty, &initial, "emitted", 65535u32.to_le_bytes().to_vec());
    let last = altered_cursor(ty, &last, "remaining", 1u32.to_le_bytes().to_vec());
    let input = step_input(step.input_type(), &last, 0, false);
    let advanced = outcome(&step.run(&input), "advanced");
    let cursor = field(&advanced, "cursor");
    assert_eq!(
        u32::from_le_bytes(leaf(field(cursor, "remaining")).try_into().unwrap()),
        0
    );
    assert_eq!(
        u32::from_le_bytes(leaf(field(cursor, "emitted")).try_into().unwrap()),
        65536
    );
    let input = step_input(step.input_type(), cursor, 0, false);
    assert_eq!(outcome(&step.run(&input), "exhausted"), *cursor);
}
#[test]
fn malformed_and_unsupported_requests_keep_different_preparation_outcomes() {
    let mut prep = Chain::new("dsp/resonator-prepare");
    for (center, bandwidth, rate, expected) in [
        ((0, 1), (100, 1), 8000, "invalid_request"),
        ((1000, 0), (100, 1), 8000, "invalid_request"),
        ((1000, 1), (100, 0), 8000, "invalid_request"),
        ((1, 1), (100, 1), 8000, "unsupported_control"),
        ((1000, 1), (1, 1), 8000, "unsupported_control"),
        ((1000, 1), (100, 1), 384000, "unsupported_control"),
    ] {
        let request = fixture::request(prep.input_type(), "", center, bandwidth, rate);
        outcome(&prep.run(&request), expected);
    }
    let request = admitted_request(prep.input_type(), 0);
    outcome(&prep.run(&request), "invalid_request");
    let request = admitted_request(prep.input_type(), 64);
    let bad_generation = altered_cursor(
        prep.input_type(),
        &request,
        "generation",
        0u64.to_le_bytes().to_vec(),
    );
    outcome(&prep.run(&bad_generation), "invalid_request");
    let new_generation = altered_cursor(
        prep.input_type(),
        &request,
        "generation",
        2u64.to_le_bytes().to_vec(),
    );
    let cursor = outcome(&prep.run(&new_generation), "started");
    assert_eq!(field(&cursor, "request"), &new_generation);
    assert_eq!(integer(&cursor, "y1"), 0);
    assert_eq!(integer(&cursor, "y2"), 0);
}
