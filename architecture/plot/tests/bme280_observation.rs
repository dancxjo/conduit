//! Source compensation composition, distinct unavailable results, and bounded reuse.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const SOURCE: &str = include_str!("../../../plots/device-protocols/main.conduit");
const STAGES: &[&str] = &[
    "begin",
    "fine",
    "temperature",
    "pressure-polynomial",
    "pressure-division",
    "pressure-check",
    "pressure-quotient",
    "pressure-correction",
    "humidity-polynomial",
    "humidity-correction",
    "humidity-clamp",
    "finish",
];
struct Prepared {
    input_type: StructuredInfoType,
    stages: Vec<PreparedPortableExpressionEvaluator>,
    first: Vec<u8>,
    second: Vec<u8>,
}
impl Prepared {
    fn new() -> Self {
        let syntax = parse_syntax_document(SOURCE);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let expanded = expand_canonical_plot_for_authoring(
            &checked,
            "bme280-observation",
            &ProfileCatalog::new(),
        )
        .unwrap()
        .expanded;
        assert_eq!(expanded.gears.len(), STAGES.len());
        let mut input_type = None;
        let stages = STAGES
            .iter()
            .map(|stage| {
                let name = format!("bme280-observation-{stage}");
                let expanded =
                    expand_canonical_plot_for_authoring(&checked, &name, &ProfileCatalog::new())
                        .unwrap()
                        .expanded;
                let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value
                else {
                    panic!("program")
                };
                let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
                if input_type.is_none() {
                    input_type = Some(program.input_type.clone());
                }
                PreparedPortableExpressionEvaluator::new(&program).unwrap()
            })
            .collect();
        Self {
            input_type: input_type.unwrap(),
            stages,
            first: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            second: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }
    fn evaluate(&mut self, input: &[u8]) -> &[u8] {
        self.first.clear();
        self.first.extend_from_slice(input);
        for stage in &mut self.stages {
            let bytes = stage.evaluate(&self.first).unwrap();
            self.second.clear();
            self.second.extend_from_slice(bytes);
            core::mem::swap(&mut self.first, &mut self.second);
        }
        &self.first
    }
    fn input(&self, pressure: i128, temperature: i128, humidity: i128, p1: i128) -> Vec<u8> {
        let calibration = record(
            field_type(&self.input_type, "calibration"),
            &[
                ("t1", 27504),
                ("t2", 26435),
                ("t3", -1000),
                ("p1", p1),
                ("p2", -10685),
                ("p3", 3024),
                ("p4", 2855),
                ("p5", 140),
                ("p6", -7),
                ("p7", 15500),
                ("p8", -14600),
                ("p9", 6000),
                ("h1", 75),
                ("h2", 362),
                ("h3", 0),
                ("h4", 315),
                ("h5", 50),
                ("h6", 30),
            ],
        );
        let sample = record(
            field_type(&self.input_type, "sample"),
            &[
                ("pressure", pressure),
                ("temperature", temperature),
                ("humidity", humidity),
            ],
        );
        StructuredInfoValue::record(
            self.input_type.clone(),
            vec![
                StructuredFieldValue::new("calibration", calibration).unwrap(),
                StructuredFieldValue::new("sample", sample).unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    }
}
fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
}
fn record(ty: &StructuredInfoType, fields: &[(&str, i128)]) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|(name, value)| {
                StructuredFieldValue::new(
                    *name,
                    StructuredInfoValue::leaf(
                        field_type(ty, name).clone(),
                        value.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn complete_compensation_yields_one_exact_typed_observation() {
    let mut prepared = Prepared::new();
    let input = prepared.input(415148, 519888, 30000, 36477);
    let output = StructuredInfoValue::from_canonical_bytes(prepared.evaluate(&input)).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = output.shape() else {
        panic!("result")
    };
    assert_eq!(tag, "observation");
    let StructuredInfoValueShape::Record(fields) = payload.shape() else {
        panic!("observation")
    };
    for (name, wanted) in [
        ("temperature_centidegrees", 2508_i128),
        ("pressure_q24_8_pa", 25767233),
        ("humidity_q22_10_percent", 55588),
    ] {
        let value = fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value();
        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
            panic!("integer")
        };
        assert_eq!(i128::from_le_bytes(bytes.try_into().unwrap()), wanted);
    }
    let (_, play) = allocation_probe::observe(|| {
        for _ in 0..1000 {
            assert!(!prepared.evaluate(&input).is_empty());
        }
    });
    assert_eq!(play.allocations, 0);
    assert_eq!(play.reallocations, 0);
}
#[test]
fn disabled_samples_and_invalid_calibration_never_publish_numeric_defaults() {
    let mut prepared = Prepared::new();
    for (p, t, h, p1, wanted) in [
        (524288, 519888, 30000, 36477, "sample-disabled"),
        (415148, 524288, 30000, 36477, "sample-disabled"),
        (415148, 519888, 32768, 36477, "sample-disabled"),
        (415148, 519888, 30000, 0, "invalid-calibration"),
        (-1, 519888, 30000, 36477, "malformed"),
        (1048576, 519888, 30000, 36477, "malformed"),
        (415148, 1048576, 30000, 36477, "malformed"),
        (415148, 519888, 65536, 36477, "malformed"),
        (415148, 519888, 30000, 65536, "malformed"),
    ] {
        let input = prepared.input(p, t, h, p1);
        let output = StructuredInfoValue::from_canonical_bytes(prepared.evaluate(&input)).unwrap();
        let StructuredInfoValueShape::Variant { tag, payload } = output.shape() else {
            panic!("result")
        };
        assert_eq!(tag, "unavailable");
        let StructuredInfoValueShape::Variant { tag, .. } = payload.shape() else {
            panic!("failure")
        };
        assert_eq!(tag, wanted);
    }
}
