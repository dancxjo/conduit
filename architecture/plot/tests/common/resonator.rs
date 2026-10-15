//! Continuous typed coefficient projection, not complete filter admission.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::*;
pub fn source() -> String {
    [
        include_str!("../../../../semantics/audio/types.conduit"),
        include_str!("../../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../../semantics/audio/rate_types.conduit"),
        include_str!("../../../../semantics/audio/decibel_types.conduit"),
        include_str!("../../../../semantics/audio/exact_control_types.conduit"),
        include_str!("../../../../semantics/audio/time_analysis_types.conduit"),
        include_str!("../../../../semantics/audio/spectral_analysis_types.conduit"),
        include_str!("../../../../semantics/audio/dsp/resonator-recurrence.conduit"),
        include_str!("../../../../semantics/audio/dsp/resonator-types.conduit"),
        include_str!("../../../../semantics/audio/dsp/resonator-projection.conduit"),
        include_str!("../../../../semantics/audio/dsp/resonator.conduit"),
    ]
    .join("\n")
}
pub fn programs(entry: &str) -> Vec<PortableExpressionProgram> {
    let source = source();
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    assert!(!authored.expanded.gears.is_empty() && authored.expanded.gears.len() <= 64);
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
pub fn request(
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
pub fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
pub fn leaf(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("leaf")
    };
    bytes
}
pub fn integer(value: &StructuredInfoValue, name: &str) -> i128 {
    i128::from_le_bytes(leaf(field(value, name)).try_into().unwrap())
}
