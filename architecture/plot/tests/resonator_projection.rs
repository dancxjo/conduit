//! Continuous typed coefficient projection, not complete filter admission.
use conduit_core::StructuredInfoValue;
#[path = "common/resonator.rs"]
mod fixture;
use fixture::{field, integer, leaf, request};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn programs() -> Vec<conduit_plot::PortableExpressionProgram> {
    fixture::programs("dsp/resonator-project")
}
use conduit_plot::PreparedPortableExpressionEvaluator;
#[test]
fn actual_checked_source_projects_fractional_controls_and_retains_originals_without_growth() {
    let programs = programs();
    let mut prepared = programs
        .iter()
        .map(|p| PreparedPortableExpressionEvaluator::new(p).unwrap())
        .collect::<Vec<_>>();
    let mut cases = vec![
        ((1000, 1), (100, 1), 8000),
        ((440, 1), (80, 1), 48000),
        ((44101, 7), (64003, 11), 96000),
        ((189000, 2), (750, 1), 192000),
        (
            (u64::MAX, u64::MAX / 1000),
            (u64::MAX, u64::MAX / 100),
            8000,
        ),
    ];
    for rate in [8000, 44100, 192000] {
        for center in [1, 32, 63] {
            for bandwidth in [1, 16, 32] {
                cases.push(((rate * center, 128), (rate * bandwidth, 256), rate));
            }
        }
    }
    // Salvaged reference vectors from recovery/5278-before-audio-split at
    // 78c38a383f4a3c6c910275f5e66e652f7744a33c, Speech resonator projection tests.
    // Keep this profile's declared center range; do not relabel the old Q20 profile.
    for rate in [8000,16000,48000] {
        for center in [1,50,270,400,730,870,1090,1500,1700,2240,2290,2440,2500,2600,3010,rate*9/20] {
            if center*128 < rate {continue;}
            for bandwidth in [10,80,100,140,rate/8] {
                cases.push(((center,1),(bandwidth,1),rate));
            }
        }
    }
    for (center, bandwidth, rate) in cases {
        let original = request(&programs[0].input_type, "", center, bandwidth, rate);
        let mut input = original.canonical_bytes().unwrap();
        for (p, evaluator) in programs.iter().zip(&mut prepared) {
            let expected = p.evaluate(&input).unwrap();
            let cap = evaluator.output_capacity();
            let (output, allocation) =
                allocation_probe::observe(|| evaluator.evaluate(&input).unwrap());
            assert_eq!(output, expected);
            assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
            input = output.to_vec();
            assert_eq!(evaluator.output_capacity(), cap);
        }
        let output = StructuredInfoValue::from_canonical_bytes(&input).unwrap();
        assert_eq!(field(&output, "request"), &original);
        assert_eq!(leaf(field(&output, "valid")), &[1], "center={center:?}, bandwidth={bandwidth:?}, rate={rate}; b={}, c={}, decay={}, cosine={}",integer(&output,"b_q14"),integer(&output,"c_q14"),integer(&output,"decay_q30"),integer(&output,"cosine_q30"));
        let b = integer(&output, "b_q14") as f64 / 16384.0;
        let c = integer(&output, "c_q14") as f64 / 16384.0;
        assert!(c > -1.0 && c < 0.0 && b.abs() < 1.0 - c);
        let radius = (-c).sqrt();
        let observed_center =
            (b / (2.0 * radius)).acos() * f64::from(rate as u32) / (2.0 * std::f64::consts::PI);
        let observed_bandwidth = -radius.ln() * f64::from(rate as u32) / std::f64::consts::PI;
        let center = center.0 as f64 / center.1 as f64;
        let bandwidth = bandwidth.0 as f64 / bandwidth.1 as f64;
        assert!((observed_center - center).abs() <= rate as f64 / 8000.0);
        assert!((observed_bandwidth - bandwidth).abs() <= rate as f64 / 20000.0);
    }
}

#[test]
fn unsupported_control_ranges_refuse_projection_before_division_or_narrowing() {
    let chain = programs();
    let carry = &chain[0];
    let p = &chain[1];
    for (center, bandwidth, rate) in [
        ((0, 1), (100, 1), 8000),
        ((1000, 0), (100, 1), 8000),
        ((1000, 1), (0, 1), 8000),
        ((1000, 1), (100, 0), 8000),
        ((1000, 1), (100, 1), 0),
        ((1000, 1), (100, 1), u64::MAX),
        ((u64::MAX, u64::MAX), (u64::MAX, u64::MAX), u64::MAX),
        ((1, 1), (100, 1), 8000),
        ((4000, 1), (100, 1), 8000),
        ((1000, 1), (1, 1), 8000),
        ((1000, 1), (2000, 1), 8000),
    ] {
        let input = request(&carry.input_type, "", center, bandwidth, rate)
            .canonical_bytes()
            .unwrap();
        let input = carry.evaluate(&input).unwrap();
        let output = p.evaluate(&input).unwrap();
        let output = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
        assert_eq!(
            leaf(field(&output, "valid")),
            &[0],
            "center={center:?},bandwidth={bandwidth:?},rate={rate}"
        );
    }
}
