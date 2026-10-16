//! Actual Source child execution after existing Audio trajectory admission.
use conduit_audio::*;
use conduit_core::*;
use conduit_plot::{
    rust_binding::{BoundedSequence, NativeRustBinding},
    PortableExpressionProgram,
};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
mod common_acoustic_reference;
#[path = "common_acoustic_value/kernel.rs"]
mod kernel;
use common_acoustic_reference::{field, prepare};

fn number(value: &StructuredInfoValue, name: &str) -> u64 {
    let value = field(value, name);
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("integer")
    };
    u64::from_le_bytes(bytes.try_into().unwrap())
}
fn request_type(plan: &Plan) -> StructuredInfoType {
    plan.fragments[0]
        .placements
        .iter()
        .find_map(|gear| {
            let ConfigurationValue::Text(encoded) = &gear.configuration.first()?.value else {
                return None;
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).ok()?;
            let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
                return None;
            };
            (fields.len() == 1 && fields[0].name() == "input").then_some(program.input_type)
        })
        .expect("the Source child consumes the admitted arithmetic request")
}
#[test]
fn authored_weight_and_blend_preserve_step_linear_and_exact_endpoint_fractions() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(check_trajectory)
        .unwrap()
        .join()
        .unwrap();
}
fn check_trajectory() {
    let anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    for policy in [
        AudioTrajectoryInterpolation::Step,
        AudioTrajectoryInterpolation::Linear,
    ] {
        let trajectory = AudioQuantityTrajectory::new(
            anchor.clone(),
            AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
            AudioTrajectoryOutside::Refuse,
            AudioTrajectoryProvenance::new(
                AudioTrajectoryProvenanceKind::Authored,
                "independent rational reference".into(),
                Some("v1".into()),
            )
            .unwrap(),
            BoundedSequence::try_from_iter([AudioTrajectorySegment::new(
                AudioExactTimeOffset::new(1, 1).unwrap(),
                policy,
                AudioTrajectoryQuantity::frequency(3, 100).unwrap(),
                AudioTrajectoryQuantity::frequency(7, 200).unwrap(),
                AudioExactTimeOffset::new(1, 0).unwrap(),
            )
            .unwrap()])
            .unwrap(),
        )
        .unwrap();
        let admitted =
            PreparedAudioQuantityTrajectory::new(&trajectory.clone().encode().unwrap()).unwrap();
        for (qn, qd) in [(0, 1), (1, 4), (1, 2), (3, 4), (1, 1)] {
            let query = AudioTrajectoryQuery::new(
                anchor.clone(),
                AudioExactTimeOffset::new(qd, qn).unwrap(),
            )
            .unwrap()
            .encode()
            .unwrap();
            let admission = admitted.evaluate(&query).unwrap();
            assert_eq!(admission.original(), &trajectory);
            assert_eq!(admission.query_canonical(), query);
            let execution = admission.executions().last().unwrap();
            let carrier =
                StructuredInfoValue::from_canonical_bytes(execution.input_canonical()).unwrap();
            let input = if policy == AudioTrajectoryInterpolation::Linear {
                field(&field(&carrier, "request"), "input")
            } else {
                field(&carrier, "input")
            };
            let (plan, front, pair) = prepare(
                include_str!("fixtures/acoustic-trajectory-value.conduit"),
                "AcousticTrajectoryStage",
                "acoustic-trajectory-value",
            );
            // Re-admit the exact same private arithmetic fields in this bounded
            // U8 profile. No quantity is narrowed, reduced or reinterpreted.
            let request = StructuredInfoValue::record(
                request_type(&plan),
                vec![StructuredFieldValue::new("input", input).unwrap()],
            )
            .unwrap();
            let encoded = request.canonical_bytes().unwrap();
            let result = kernel::run(plan, &front, pair, vec![], encoded, false).unwrap();
            let result = StructuredInfoValue::from_canonical_bytes(&result).unwrap();
            assert_eq!(field(&result, "original"), request);
            let ratio = field(&result, "result");
            let n = u128::from(number(&ratio, "numerator"));
            let d = u128::from(number(&ratio, "denominator"));
            let (rn, rd) = if qn == qd {
                (200, 7)
            } else if qn == 0 || policy == AudioTrajectoryInterpolation::Step {
                (100, 3)
            } else {
                (
                    (u128::from(qd - qn) * 100 * 7) + (u128::from(qn) * 200 * 3),
                    u128::from(qd) * 3 * 7,
                )
            };
            assert_eq!(n * rd, rn * d);
            let domain = field(&ratio, "domain");
            let StructuredInfoValueShape::Variant { tag, .. } = domain.shape() else {
                panic!("domain")
            };
            assert_eq!(tag, "frequency");
            if qn == 0 {
                assert_eq!((n, d), (100, 3));
            }
            if qn == qd {
                assert_eq!((n, d), (200, 7));
            }
        }
    }
}
