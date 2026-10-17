//! Single-source checked workloads; the warm reference is the production
//! prepared evaluator, with its ordinary decode/validation/overflow behavior.
use crate::{
    config::Config,
    timing::{self, Metrics},
};
use conduit_plot::{
    BackStatement, CheckedExpressionType, CordStage, ExpressionTypeContext,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    hint::black_box,
    time::Instant,
};

#[derive(Debug, Clone, Copy)]
pub enum Domain {
    U64,
    Scalar,
}
#[derive(Debug, Clone, Copy)]
pub enum Shape {
    Add(usize),
    Conditional,
    Comparison,
}
#[derive(Debug, Clone, Copy)]
pub struct Workload {
    pub domain: Domain,
    pub shape: Shape,
}
impl Workload {
    pub fn id(self) -> String {
        format!("{:?}-{:?}", self.domain, self.shape)
            .to_lowercase()
            .replace(['(', ')'], "")
    }
    fn scale(self) -> u64 {
        match self.domain {
            Domain::U64 => 1,
            Domain::Scalar => conduit_core::Scalar::SCALE as u64,
        }
    }
    pub fn input(self, index: u32) -> u64 {
        u64::from(index) * self.scale()
    }
    pub fn operations(self) -> u64 {
        match self.shape {
            Shape::Add(n) => n as u64,
            Shape::Conditional => 2,
            Shape::Comparison => 1,
        }
    }
    pub fn source(self) -> String {
        let ty = match self.domain {
            Domain::U64 => "U64",
            Domain::Scalar => "Scalar",
        };
        let output = if matches!(self.shape, Shape::Comparison) {
            "Bool"
        } else {
            ty
        };
        let expression = match self.shape {
            Shape::Add(n) => addition_source(n),
            Shape::Conditional => ". > 1 ? . + 1 : . + 2".into(),
            Shape::Comparison => ". > 1".into(),
        };
        format!("plot baseline (\n input: {ty} >> output: {output}\n) {{\n input >> ({expression}) >> output\n}}\n")
    }
    /// Hand-written checked native law. Each selected addition is checked;
    /// the compiler may optimize the equivalent computation, as a native baseline.
    pub fn native(self, value: u64) -> Option<u64> {
        match self.shape {
            Shape::Add(1) => add::<1>(value, self),
            Shape::Add(3) => add::<3>(value, self),
            Shape::Add(8) => add::<8>(value, self),
            Shape::Add(32) => add::<32>(value, self),
            Shape::Add(_) => None,
            Shape::Conditional => checked_add(
                value,
                if self.greater(value) {
                    self.scale()
                } else {
                    2 * self.scale()
                },
                self.domain,
            ),
            Shape::Comparison => Some(u64::from(match self.domain {
                Domain::U64 => value > self.scale(),
                Domain::Scalar => (value as i64) > self.scale() as i64,
            })),
        }
    }
    fn greater(self, value: u64) -> bool {
        match self.domain {
            Domain::U64 => value > self.scale(),
            Domain::Scalar => (value as i64) > self.scale() as i64,
        }
    }
    pub fn decode(self, bytes: &[u8]) -> Result<u64, String> {
        if matches!(self.shape, Shape::Comparison) {
            return conduit_core::InfoBool::decode(bytes)
                .map(|v| u64::from(v.get()))
                .map_err(|e| format!("Bool output: {e:?}"));
        }
        let frame: [u8; 8] = bytes.try_into().map_err(|_| "wrong output frame")?;
        Ok(u64::from_le_bytes(frame))
    }
}
/// Keep the fixed number of checked additions within the portable tree-depth
/// profile. The right subtree contains n constants and n-1 additions.
pub(crate) fn addition_source(n: usize) -> String {
    fn ones(n: usize) -> String {
        if n == 1 {
            "1".into()
        } else {
            format!("({} + {})", ones(n / 2), ones(n - n / 2))
        }
    }
    assert!(n > 0);
    format!(". + {}", ones(n))
}
fn portable_roundtrip(
    program: PortableExpressionProgram,
) -> Result<PortableExpressionProgram, String> {
    let encoded = program
        .canonical_hex()
        .map_err(|e| format!("encode: {e:?}"))?;
    let decoded = PortableExpressionProgram::from_canonical_hex(&encoded)
        .map_err(|e| format!("decode: {e:?}"))?;
    if decoded != program {
        return Err("portable expression roundtrip changed the program".into());
    }
    Ok(decoded)
}
fn checked_add(value: u64, delta: u64, domain: Domain) -> Option<u64> {
    match domain {
        Domain::U64 => value.checked_add(delta),
        Domain::Scalar => (value as i64).checked_add(delta as i64).map(|v| v as u64),
    }
}
fn add<const N: usize>(mut value: u64, workload: Workload) -> Option<u64> {
    for _ in 0..N {
        value = checked_add(value, workload.scale(), workload.domain)?;
    }
    Some(value)
}
fn checked(
    document: &conduit_plot::SyntaxDocument,
    workload: Workload,
) -> Result<conduit_plot::CheckedExpression, String> {
    if !document.diagnostics.is_empty() {
        return Err(format!("parse: {:?}", document.diagnostics));
    }
    let BackStatement::Cord(cord) = &document.plots[0].back[0] else {
        return Err("missing fixture Cord".into());
    };
    let CordStage::PureExpression(expression) = &cord.stages[1] else {
        return Err("missing fixture expression".into());
    };
    let input = CheckedExpressionType::semantic(match workload.domain {
        Domain::U64 => "value/u64",
        Domain::Scalar => conduit_core::SCALAR_INFO_ID,
    });
    conduit_plot::check_expression(
        &expression.syntax,
        &ExpressionTypeContext {
            glyph_values: None,
            input: &input,
            immutable_values: &BTreeMap::new(),
            structured_types: &BTreeMap::new(),
            literal_types: &BTreeMap::new(),
            numeric_types: &BTreeSet::new(),
            semantic_kinds: &BTreeMap::new(),
        },
    )
    .map_err(|e| format!("check: {e:?}"))
}
#[cfg(test)]
pub fn program(workload: Workload) -> Result<PortableExpressionProgram, String> {
    let document = conduit_plot::parse_syntax_document(&workload.source());
    portable_roundtrip(
        PortableExpressionProgram::from_checked(&checked(&document, workload)?)
            .map_err(|e| format!("program: {e:?}"))?,
    )
}
#[derive(serde::Serialize)]
pub struct Cold {
    pub parse: Metrics,
    pub check_expression: Metrics,
    pub compile_checked_expression: Metrics,
    pub canonical_codec: Metrics,
    pub prepare: Metrics,
    pub first_output: Metrics,
    pub teardown: Metrics,
}
pub fn cold(workload: Workload) -> Result<(PortableExpressionProgram, Cold), String> {
    let source = workload.source();
    let (document, parse) = timing::measure(|| conduit_plot::parse_syntax_document(&source));
    let (value, check_expression) = timing::measure(|| checked(&document, workload));
    let value = value?;
    let (program, compile_checked_expression) =
        timing::measure(|| PortableExpressionProgram::from_checked(&value));
    let program = program.map_err(|e| format!("compile: {e:?}"))?;
    let (program, canonical_codec) = timing::measure(|| portable_roundtrip(program));
    let program = program?;
    let (evaluator, prepare) =
        timing::measure(|| PreparedPortableExpressionEvaluator::new(&program));
    let mut evaluator = evaluator.map_err(|e| format!("prepare: {e:?}"))?;
    let input = workload.input(42).to_le_bytes();
    let (output, first_output) =
        timing::measure(|| evaluator.evaluate(&input).map(|b| workload.decode(b)));
    if output.map_err(|e| format!("first output: {e:?}"))??
        != workload.native(workload.input(42)).unwrap()
    {
        return Err("first output differs from native law".into());
    }
    let (_, teardown) = timing::measure(|| drop(evaluator));
    Ok((
        program,
        Cold {
            parse,
            check_expression,
            compile_checked_expression,
            canonical_codec,
            prepare,
            first_output,
            teardown,
        },
    ))
}
#[derive(serde::Serialize)]
pub struct Warm {
    pub mode: &'static str,
    pub values: u32,
    pub primitive_operations: u64,
    pub validation_sum: String,
    pub validation_sha256: String,
    pub output_validation: Metrics,
    pub metrics: Metrics,
    pub latency_ns: Vec<u64>,
    pub latency_distribution: timing::Distribution,
    pub sampling: &'static str,
}
pub fn warm(
    workload: Workload,
    program: &PortableExpressionProgram,
    config: &Config,
    prepared: bool,
) -> Result<Warm, String> {
    let mut evaluator =
        PreparedPortableExpressionEvaluator::new(program).map_err(|e| format!("prepare: {e:?}"))?;
    let mut evaluate = |index: u32| -> Result<u64, String> {
        let input = black_box(workload.input(index));
        if prepared {
            workload.decode(black_box(
                evaluator
                    .evaluate(black_box(&input.to_le_bytes()))
                    .map_err(|e| format!("warm arithmetic: {e:?}"))?,
            ))
        } else {
            workload
                .native(input)
                .ok_or_else(|| "native overflow".into())
        }
    };
    for index in 0..10_000 {
        black_box(evaluate(index)?);
    }
    let (sum, metrics) = timing::measure(|| -> Result<u128, String> {
        let mut sum = 0_u128;
        for index in 0..config.values {
            sum += u128::from(black_box(evaluate(index)?));
        }
        Ok(sum)
    });
    let sum = sum?;
    // A separate uniform-stride pass does not put per-item clock calls in the
    // bulk throughput loop. Record its distinct instrumentation policy.
    let mut latency_ns = Vec::with_capacity(config.samples as usize);
    for index in 0..config.samples {
        let item = u64::from(index) * u64::from(config.values) / u64::from(config.samples);
        let start = Instant::now();
        black_box(evaluate(item as u32)?);
        latency_ns.push(timing::ns(start.elapsed()));
    }
    let latency_distribution = timing::distribution(&latency_ns);
    // This separately measured pass checks every output against the native law
    // and hashes the ordered normalized u64 coordinates. It is outside both
    // bulk throughput and latency clocks, never hidden inside their metrics.
    let (validation, output_validation) = timing::measure(|| -> Result<_, String> {
        let mut digest = Sha256::new();
        for index in 0..config.values {
            let value = evaluate(index)?;
            if Some(value) != workload.native(workload.input(index)) {
                return Err(format!("wrong output at input {index}"));
            }
            digest.update(value.to_le_bytes());
        }
        Ok(digest.finalize())
    });
    let validation_sha256 = format!("{:x}", validation?);
    Ok(Warm { mode: if prepared { "prepared-reference" } else { "native-optimized" },
        values: config.values, primitive_operations: u64::from(config.values) * workload.operations(),
        validation_sum: sum.to_string(), validation_sha256, output_validation, metrics, latency_ns, latency_distribution,
        sampling: "bulk has no per-item timer; separate warm uniform-stride latency pass and separately measured full output-law/digest validation pass; clocks not subtracted" })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn additions(node: &conduit_plot::PortableExpressionNode) -> usize {
        match &node.operation {
            conduit_plot::PortableExpressionOperation::Binary {
                operator: conduit_plot::BinaryOperator::Add,
                left,
                right,
                ..
            } => 1 + additions(left) + additions(right),
            conduit_plot::PortableExpressionOperation::Input
            | conduit_plot::PortableExpressionOperation::Literal(_) => 0,
            other => panic!("unexpected addition workload operation: {other:?}"),
        }
    }
    #[test]
    fn frozen_sources_match_native_outputs_branches_and_refusals() {
        for domain in [Domain::U64, Domain::Scalar] {
            for shape in [
                Shape::Add(1),
                Shape::Add(3),
                Shape::Add(8),
                Shape::Add(32),
                Shape::Conditional,
                Shape::Comparison,
            ] {
                let workload = Workload { domain, shape };
                let program = program(workload).unwrap();
                if let Shape::Add(n) = shape {
                    assert_eq!(additions(&program.root), n);
                }
                let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
                let maximum = match domain {
                    Domain::U64 => u64::MAX,
                    Domain::Scalar => i64::MAX as u64,
                };
                let minimum = match domain {
                    Domain::U64 => 0,
                    Domain::Scalar => i64::MIN as u64,
                };
                let delta = match shape {
                    Shape::Add(n) => n as u64 * workload.scale(),
                    _ => workload.scale(),
                };
                for input in [
                    maximum - delta,
                    maximum - delta + 1,
                    0,
                    workload.input(1),
                    workload.input(2),
                    workload.input(99),
                    maximum,
                    minimum,
                ] {
                    let native = workload.native(input);
                    let prepared = evaluator.evaluate(&input.to_le_bytes());
                    assert_eq!(
                        prepared.is_ok(),
                        native.is_some(),
                        "{} at {input}",
                        workload.id()
                    );
                    if let Some(expected) = native {
                        assert_eq!(workload.decode(prepared.unwrap()).unwrap(), expected);
                    }
                }
                assert!(evaluator.evaluate(&[1, 2, 3]).is_err());
            }
        }
    }
}
