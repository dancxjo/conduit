//! The original authored stages are the oracle, not a second Rust formula.
use conduit_bosch_env::bme280_observation::{Calibration, Error, Observation, Sample, Unavailable};
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionEvaluationRefusal, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
const SOURCE: &str = include_str!("../../../../../plots/device-protocols/main.conduit");
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
pub struct Prepared {
    input_type: StructuredInfoType,
    stages: Vec<PreparedPortableExpressionEvaluator>,
    first: Vec<u8>,
    second: Vec<u8>,
}
impl Prepared {
    pub fn new() -> Self {
        let syntax = parse_syntax_document(SOURCE);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let profile = ProfileCatalog::new();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, "bme280-observation", &profile)
                .unwrap()
                .expanded;
        assert_eq!(expanded.gears.len(), STAGES.len());
        let mut input_type = None;
        let stages = STAGES
            .iter()
            .map(|stage| {
                let name = format!("bme280-observation-{stage}");
                let expanded = expand_canonical_plot_for_authoring(&checked, &name, &profile)
                    .unwrap()
                    .expanded;
                let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value
                else {
                    panic!("authored expression program")
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
    pub fn input(&self, c: Calibration, s: Sample) -> Vec<u8> {
        let calibration = record(
            field_type(&self.input_type, "calibration"),
            &[
                ("t1", c.t1),
                ("t2", c.t2),
                ("t3", c.t3),
                ("p1", c.p1),
                ("p2", c.p2),
                ("p3", c.p3),
                ("p4", c.p4),
                ("p5", c.p5),
                ("p6", c.p6),
                ("p7", c.p7),
                ("p8", c.p8),
                ("p9", c.p9),
                ("h1", c.h1),
                ("h2", c.h2),
                ("h3", c.h3),
                ("h4", c.h4),
                ("h5", c.h5),
                ("h6", c.h6),
            ],
        );
        let sample = record(
            field_type(&self.input_type, "sample"),
            &[
                ("pressure", s.pressure),
                ("temperature", s.temperature),
                ("humidity", s.humidity),
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
    pub fn canonical_result(&self) -> &[u8] {
        &self.first
    }
    pub fn evaluate(&mut self, c: Calibration, s: Sample) -> Result<Observation, Error> {
        let input = self.input(c, s);
        self.first.clear();
        self.first.extend_from_slice(&input);
        for (name, stage) in STAGES.iter().zip(&mut self.stages) {
            let bytes = match stage.evaluate(&self.first) {
                Ok(bytes) => bytes,
                Err(PortableExpressionEvaluationRefusal::Arithmetic) => {
                    return Err(Error::Arithmetic)
                }
                Err(other) => panic!("unexpected Source evaluator refusal in {name}: {other:?}"),
            };
            self.second.clear();
            self.second.extend_from_slice(bytes);
            core::mem::swap(&mut self.first, &mut self.second);
        }
        decode(self.canonical_result())
    }
}
fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record type")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
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
fn decode(bytes: &[u8]) -> Result<Observation, Error> {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("result")
    };
    match tag {
        "observation" => {
            let StructuredInfoValueShape::Record(fields) = payload.shape() else {
                panic!("observation")
            };
            let coordinate = |name| {
                let value = fields.iter().find(|f| f.name() == name).unwrap().value();
                let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                    panic!("I128")
                };
                i128::from_le_bytes(bytes.try_into().unwrap())
            };
            Ok(Observation {
                temperature_centidegrees: coordinate("temperature_centidegrees"),
                pressure_q24_8_pa: coordinate("pressure_q24_8_pa"),
                humidity_q22_10_percent: coordinate("humidity_q22_10_percent"),
            })
        }
        "unavailable" => {
            let StructuredInfoValueShape::Variant { tag, .. } = payload.shape() else {
                panic!("unavailable")
            };
            Err(Error::Unavailable(match tag {
                "malformed" => Unavailable::Malformed,
                "sample-disabled" => Unavailable::SampleDisabled,
                "invalid-calibration" => Unavailable::InvalidCalibration,
                other => panic!("unexpected unavailable {other}"),
            }))
        }
        other => panic!("unexpected result {other}"),
    }
}
