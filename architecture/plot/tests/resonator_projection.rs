//! Continuous typed coefficient projection, not complete filter admission.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn programs() -> Vec<PortableExpressionProgram> {
    let source = [
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../semantics/audio/rate_types.conduit"),
        include_str!("../../../semantics/audio/decibel_types.conduit"),
        include_str!("../../../semantics/audio/exact_control_types.conduit"),
        include_str!("../../../semantics/audio/time_analysis_types.conduit"),
        include_str!("../../../semantics/audio/spectral_analysis_types.conduit"),
        include_str!("../../../semantics/audio/dsp/resonator-projection.conduit"),
    ]
    .join("\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let authored = expand_canonical_plot_for_authoring(
        &checked,
        "dsp/resonator-project",
        &ProfileCatalog::new(),
    )
    .unwrap();
    assert_eq!(authored.expanded.gears.len(), 22);
    // Follow the actual unary Cords: canonical gear storage is not execution order.
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 1);
    let mut id = authored.input_bindings[0].gear_id.clone();
    let mut ordered = Vec::new();
    let mut seen = Vec::new();
    loop {
        assert!(!seen.contains(&id), "acyclic preparation graph");
        seen.push(id.clone());
        let gear = authored
            .expanded
            .gears
            .iter()
            .find(|g| g.gear_id == id)
            .unwrap();
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let p = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        if let Some(previous) = ordered.last() {
            let previous: &PortableExpressionProgram = previous;
            assert_eq!(previous.output_type, p.input_type);
        }
        ordered.push(p);
        let outgoing = authored
            .expanded
            .connections
            .iter()
            .filter(|c| c.source_gear_id == id)
            .collect::<Vec<_>>();
        if outgoing.is_empty() {
            assert_eq!(authored.output_bindings[0].gear_id, id);
            break;
        }
        assert_eq!(outgoing.len(), 1);
        id = outgoing[0].sink_gear_id.clone();
    }
    assert_eq!(ordered.len(), authored.expanded.gears.len());
    ordered
}
fn request(
    ty: &StructuredInfoType,
    path: &str,
    center: (u64, u64),
    bandwidth: (u64, u64),
    rate: u64,
) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|f| {
                    StructuredFieldValue::new(
                        f.name(),
                        request(
                            f.value_type(),
                            &format!("{path}.{}", f.name()),
                            center,
                            bandwidth,
                            rate,
                        ),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Variant { cases, .. } => {
            let c = &cases[0];
            StructuredInfoValue::variant(
                ty.clone(),
                c.tag(),
                StructuredInfoValue::leaf(c.payload_type().clone(), vec![]).unwrap(),
            )
            .unwrap()
        }
        StructuredInfoTypeShape::Leaf { .. } => {
            let bytes = if path.ends_with(".frames") {
                64u32.to_le_bytes().to_vec()
            } else {
                let v = if path.ends_with(".sample_rate_hz") {
                    rate
                } else if path.contains(".center.") {
                    if path.ends_with(".denominator") {
                        center.1
                    } else {
                        center.0
                    }
                } else if path.contains(".bandwidth.") {
                    if path.ends_with(".denominator") {
                        bandwidth.1
                    } else {
                        bandwidth.0
                    }
                } else {
                    1
                };
                v.to_le_bytes().to_vec()
            };
            StructuredInfoValue::leaf(ty.clone(), bytes).unwrap()
        }
        _ => panic!("unexpected request shape"),
    }
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn leaf(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("leaf")
    };
    bytes
}
fn integer(value: &StructuredInfoValue, name: &str) -> i128 {
    i128::from_le_bytes(leaf(field(value, name)).try_into().unwrap())
}
#[test]
fn actual_checked_source_projects_fractional_controls_and_retains_originals_without_growth() {
    let programs = programs();
    let mut prepared = programs
        .iter()
        .map(|p| PreparedPortableExpressionEvaluator::new(p).unwrap())
        .collect::<Vec<_>>();
    let mut cases = vec![
        ((1000, 1), (100, 1), 8000),
        ((440, 1), (240, 1), 48000),
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
    let p = programs().remove(0);
    for (center, bandwidth, rate) in [
        ((0, 1), (100, 1), 8000),
        ((1000, 0), (100, 1), 8000),
        ((1000, 1), (0, 1), 8000),
        ((1000, 1), (100, 0), 8000),
        ((1000, 1), (100, 1), 0),
        ((1000, 1), (100, 1), u64::MAX),
        ((1, 1), (100, 1), 8000),
        ((4000, 1), (100, 1), 8000),
        ((1000, 1), (1, 1), 8000),
        ((1000, 1), (2000, 1), 8000),
    ] {
        let input = request(&p.input_type, "", center, bandwidth, rate)
            .canonical_bytes()
            .unwrap();
        let output = p.evaluate(&input).unwrap();
        let output = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
        assert_eq!(
            leaf(field(&output, "valid")),
            &[0],
            "center={center:?},bandwidth={bandwidth:?},rate={rate}"
        );
    }
}
