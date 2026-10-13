//! Engine-owned verified evidence; callers cannot construct a verified witness.
mod checked_i64;
mod codec;
mod comparison;
mod operational;
mod symbolic;
mod uniform;
use super::{
    Binary64Flags, Binary64Operation, Binary64Value, NumericRealizationWork, RationalInterval,
};
use crate::numeric_projection::{
    NumericAccuracy, NumericRepresentation, NumericRequirement, NumericSemantics,
};
use crate::{
    MathValue, NumericComputation, NumericComputationOperation, NumericOperation, NumericRefusal,
};
use alloc::vec::Vec;
use comparison::certify_exact_boolean;
use core::cmp::Ordering;
use uniform::certify_interval;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericEngineRefusal {
    Arithmetic(NumericRefusal),
    InvalidComputation,
    UnsupportedOperation(NumericOperation),
    UnsupportedPhysicalLaw,
    UnsupportedRepresentation,
    UnsupportedRange,
    MissingParameter,
    NonFiniteMathematicalResult,
    AccuracyUnproved,
    ExceptionPolicy,
    ClaimMismatch,
    IntegerOverflow,
    InexactInteger,
}
impl From<NumericRefusal> for NumericEngineRefusal {
    fn from(value: NumericRefusal) -> Self {
        Self::Arithmetic(value)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binary64StepEvidence {
    pub node: u16,
    pub operands: Vec<u64>,
    pub output: u64,
    pub flags: Binary64Flags,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct I64StepEvidence {
    pub node: u16,
    pub operands: Vec<i64>,
    pub output: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanStepEvidence {
    pub node: u16,
    pub operands: Vec<u64>,
    pub output: bool,
    pub flags: Binary64Flags,
}
mod evidence;
mod memory;
pub use evidence::*;
fn is_math_kind(kind: &str) -> bool {
    kind == crate::MATH_VALUE_INFO_ID || crate::PortableNumericProfile::from_info_id(kind).is_some()
}

fn admit_math_value(kind: &str, value: &MathValue) -> Result<(), NumericEngineRefusal> {
    if let Some(profile) = crate::PortableNumericProfile::from_info_id(kind) {
        profile.validate(&value.canonical_bytes()?)?;
    } else if kind != crate::MATH_VALUE_INFO_ID {
        return Err(NumericEngineRefusal::UnsupportedRepresentation);
    }
    Ok(())
}

fn math_apply(
    operation: NumericOperation,
    values: &[MathValue],
) -> Result<MathValue, NumericEngineRefusal> {
    let a = &values[0];
    Ok(match operation {
        NumericOperation::Negate => a.checked_neg()?,
        NumericOperation::Add => a.checked_add(&values[1])?,
        NumericOperation::Subtract => a.checked_sub(&values[1])?,
        NumericOperation::Multiply => a.checked_mul(&values[1])?,
        NumericOperation::Divide => a.checked_div(&values[1])?,
        NumericOperation::SquareRoot => a.checked_root(2)?,
        NumericOperation::Power => {
            a.checked_pow(values[1].exact_i64().ok_or(NumericRefusal::Domain)?)?
        }
        NumericOperation::Root => a.checked_root(
            u8::try_from(values[1].exact_i64().ok_or(NumericRefusal::Domain)?)
                .map_err(|_| NumericRefusal::Domain)?,
        )?,
        _ => return Err(NumericEngineRefusal::UnsupportedOperation(operation)),
    })
}
fn binary_operation(op: NumericOperation) -> Result<Binary64Operation, NumericEngineRefusal> {
    Ok(match op {
        NumericOperation::Negate => Binary64Operation::Negate,
        NumericOperation::Add => Binary64Operation::Add,
        NumericOperation::Subtract => Binary64Operation::Subtract,
        NumericOperation::Multiply => Binary64Operation::Multiply,
        NumericOperation::Divide => Binary64Operation::Divide,
        NumericOperation::SquareRoot => Binary64Operation::SquareRoot,
        _ => return Err(NumericEngineRefusal::UnsupportedOperation(op)),
    })
}
fn absolute(value: &MathValue) -> Result<MathValue, NumericRefusal> {
    if value.compare(&MathValue::integer(0))? == Some(Ordering::Less) {
        value.checked_neg()
    } else {
        Ok(value.clone())
    }
}
fn prove_accuracy(
    exact: &MathValue,
    error: &MathValue,
    accuracy: &NumericAccuracy,
) -> Result<(), NumericEngineRefusal> {
    let tolerance = match accuracy {
        NumericAccuracy::Exact => MathValue::integer(0),
        NumericAccuracy::Absolute(value) => value.clone(),
        NumericAccuracy::Relative {
            relative,
            zero_absolute,
        } => {
            if exact.compare(&MathValue::integer(0))? == Some(Ordering::Equal) {
                zero_absolute.clone()
            } else {
                absolute(exact)?.checked_mul(relative)?
            }
        }
        // A result interval's width is not an error allowance.
        _ => return Err(NumericEngineRefusal::AccuracyUnproved),
    };
    match absolute(error)?.compare(&tolerance)? {
        Some(Ordering::Less | Ordering::Equal) => Ok(()),
        _ => Err(NumericEngineRefusal::AccuracyUnproved),
    }
}
/// Closed computations and singleton ranges are certified uniformly. Wider ranges
/// are refused until a uniform ordered-rounding enclosure algorithm is selected.
pub fn certify_computation(
    graph: &NumericComputation,
    requirement: &NumericRequirement,
) -> Result<VerifiedNumericWitness, NumericEngineRefusal> {
    graph
        .validate()
        .map_err(|_| NumericEngineRefusal::InvalidComputation)?;
    requirement.validate()?;
    if requirement.representation == NumericRepresentation::Symbolic {
        return symbolic::certify_symbolic(graph, requirement);
    }
    if requirement.representation == NumericRepresentation::CheckedI64 {
        return checked_i64::certify_i64(graph, requirement);
    }
    if requirement.semantics == NumericSemantics::IeeeOperational {
        return operational::certify_ieee(graph, requirement);
    }
    if graph.nodes()[usize::from(graph.root())].semantic_kind == crate::BOOL_INFO_ID {
        return certify_exact_boolean(graph, requirement);
    }
    if requirement.representation == NumericRepresentation::CertifiedInterval {
        return certify_interval(graph, requirement);
    }
    if requirement.semantics != NumericSemantics::Mathematical
        || !matches!(
            requirement.representation,
            NumericRepresentation::ExactRational | NumericRepresentation::Binary64
        )
    {
        return Err(NumericEngineRefusal::UnsupportedRepresentation);
    }
    // Core arithmetic primitives presently use their fixed maximum limb workspace.
    // A smaller requested workspace is not silently treated as equivalent.
    if usize::from(requirement.budget.limbs) != crate::MAX_INTEGER_LIMBS {
        return Err(NumericRefusal::IntegerBudget.into());
    }
    let mut work = NumericRealizationWork::new(requirement.budget.work_steps);
    let mut exact_values: Vec<MathValue> = Vec::with_capacity(graph.nodes().len());
    let mut binary_values: Vec<Binary64Value> = Vec::with_capacity(graph.nodes().len());
    let mut steps = Vec::with_capacity(graph.nodes().len());
    let mut flags = Binary64Flags::default();
    let mut required_symbolic_nodes = 0;
    let mut storage = graph
        .canonical_bytes()
        .map_err(|_| NumericEngineRefusal::InvalidComputation)?
        .len();
    storage += requirement.canonical_bytes()?.len();
    // Conservative peak envelope: exact arithmetic has bounded fixed-array
    // primitive scratch, plus retained values, step metadata and serialization.
    // This is a preparation-time reservation, not a Play allocation guarantee.
    storage += 32 * core::mem::size_of::<crate::ExactRational>();
    storage += graph.nodes().len()
        * (core::mem::size_of::<MathValue>() + core::mem::size_of::<Binary64StepEvidence>() + 8);
    storage += crate::MAX_NUMERIC_COMPUTATION_BYTES + crate::MAX_NUMERIC_ENCODED_BYTES;
    if storage > requirement.budget.storage_bytes as usize {
        return Err(NumericRefusal::StorageBudget.into());
    }

    for (index, node) in graph.nodes().iter().enumerate() {
        work.charge(1)?;
        if node.unit.is_some()
            || node.reference_unit.is_some()
            || !is_math_kind(&node.semantic_kind)
        {
            return Err(NumericEngineRefusal::UnsupportedPhysicalLaw);
        }
        if let NumericComputationOperation::Apply { operands, .. } = &node.operation {
            if operands
                .iter()
                .any(|index| matches!(exact_values[usize::from(*index)], MathValue::Symbolic(_)))
            {
                // The shared normalized-DAG builder has a fixed reviewed cap;
                // no narrower transient builder profile is claimed here.
                required_symbolic_nodes = crate::MAX_SYMBOLIC_NODES as u16;
                if required_symbolic_nodes > requirement.budget.symbolic_nodes {
                    return Err(NumericRefusal::NodeBudget.into());
                }
            }
        }
        let mut operand_bits = Vec::new();
        let exact = match &node.operation {
            NumericComputationOperation::IeeeLiteral { .. }
            | NumericComputationOperation::IeeeRoundedLiteral { .. } => {
                return Err(NumericEngineRefusal::UnsupportedRepresentation)
            }
            NumericComputationOperation::Literal(bytes) => MathValue::decode(bytes)?,
            NumericComputationOperation::Parameter(slot) => {
                let range = requirement
                    .parameters
                    .iter()
                    .find(|range| range.slot == *slot)
                    .ok_or(NumericEngineRefusal::MissingParameter)?;
                if range.lower.compare(&range.upper)? != Some(Ordering::Equal) {
                    return Err(NumericEngineRefusal::UnsupportedRange);
                }
                range.lower.clone()
            }
            NumericComputationOperation::Apply {
                operation,
                operands,
            } => math_apply(
                *operation,
                &operands
                    .iter()
                    .map(|index| exact_values[usize::from(*index)].clone())
                    .collect::<Vec<_>>(),
            )?,
        };
        if exact.as_rational().is_none()
            && requirement.representation != NumericRepresentation::Binary64
        {
            return Err(NumericEngineRefusal::AccuracyUnproved);
        }
        if let MathValue::Symbolic(dag) = &exact {
            required_symbolic_nodes = required_symbolic_nodes.max(dag.node_count() as u16);
            if required_symbolic_nodes > requirement.budget.symbolic_nodes {
                return Err(NumericRefusal::NodeBudget.into());
            }
        }
        admit_math_value(&node.semantic_kind, &exact)?;
        storage += exact.canonical_bytes()?.len() + core::mem::size_of::<crate::ExactRational>();
        if requirement.representation == NumericRepresentation::Binary64 {
            let result = match &node.operation {
                NumericComputationOperation::Apply {
                    operation,
                    operands,
                } => {
                    operand_bits = operands
                        .iter()
                        .map(|index| binary_values[usize::from(*index)].bits())
                        .collect();
                    binary_operation(*operation)?.apply(
                        binary_values[usize::from(operands[0])],
                        operands
                            .get(1)
                            .map(|index| binary_values[usize::from(*index)]),
                        requirement.rounding,
                        &mut work,
                    )?
                }
                _ => super::enclosure::project_mathematical_literal(
                    &exact,
                    requirement.rounding,
                    &mut work,
                )?,
            };
            if result.value.exact()?.is_none() {
                return Err(NumericEngineRefusal::NonFiniteMathematicalResult);
            }
            flags = flags.union(result.flags);
            storage += core::mem::size_of::<Binary64StepEvidence>() + operand_bits.len() * 8 + 8;
            steps.push(Binary64StepEvidence {
                node: index as u16,
                operands: operand_bits,
                output: result.value.bits(),
                flags: result.flags,
            });
            binary_values.push(result.value);
        }
        if storage > requirement.budget.storage_bytes as usize {
            return Err(NumericRefusal::StorageBudget.into());
        }
        exact_values.push(exact);
    }
    if !requirement.exceptions.permits(flags) {
        return Err(NumericEngineRefusal::ExceptionPolicy);
    }
    let exact = &exact_values[usize::from(graph.root())];
    let observed = if requirement.representation == NumericRepresentation::Binary64 {
        binary_values[usize::from(graph.root())]
            .exact()?
            .ok_or(NumericEngineRefusal::NonFiniteMathematicalResult)?
    } else {
        exact.clone()
    };
    let reference = crate::numeric_realization::enclose_math_value(exact, &mut work)?;
    let result = RationalInterval::point(observed)?;
    let error = result.subtract(&reference)?;
    if let NumericAccuracy::Ulps(maximum) = requirement.accuracy {
        let lower =
            Binary64Value::project(reference.lower(), requirement.rounding, &mut work)?.value;
        let upper =
            Binary64Value::project(reference.upper(), requirement.rounding, &mut work)?.value;
        let key = |value: Binary64Value| {
            if value.is_negative() {
                (!value.bits()).wrapping_add(1)
            } else {
                value.bits() | (1 << 63)
            }
        };
        if !lower.is_infinite() && !upper.is_infinite() && key(lower) == key(upper) {
            let observed = binary_values[usize::from(graph.root())];
            if key(observed).abs_diff(key(lower)) > u64::from(maximum) {
                return Err(NumericEngineRefusal::AccuracyUnproved);
            }
        } else {
            return Err(NumericEngineRefusal::AccuracyUnproved);
        }
    } else {
        prove_accuracy(exact, error.lower(), &requirement.accuracy)?;
        prove_accuracy(exact, error.upper(), &requirement.accuracy)?;
    }
    storage +=
        result.lower().canonical_bytes()?.len() * 2 + error.lower().canonical_bytes()?.len() * 2;
    if storage > requirement.budget.storage_bytes as usize {
        return Err(NumericRefusal::StorageBudget.into());
    }
    Ok(VerifiedNumericWitness {
        requirement: requirement.clone(),
        claim: NumericCertificateClaim {
            engine_identity: crate::numeric_realization::numeric_engine_identity(),
            graph_identity: graph
                .identity()
                .map_err(|_| NumericEngineRefusal::InvalidComputation)?,
            algorithm: if requirement.representation == NumericRepresentation::Binary64 {
                NumericAlgorithm::OrderedSoftwareBinary64V1
            } else {
                NumericAlgorithm::OrderedRationalV1
            },
            requirement_identity: requirement.identity()?,
            result: if requirement.representation == NumericRepresentation::ExactRational {
                NumericResultEvidence::ExactMathematical(exact.clone())
            } else {
                NumericResultEvidence::Enclosure(result)
            },
            error,
            steps,
            boolean_steps: Vec::new(),
            integer_steps: Vec::new(),
            flags,
            work_steps: work.used(),
            storage_bytes: storage as u32,
            required_limb_capacity: crate::MAX_INTEGER_LIMBS as u16,
            required_symbolic_nodes,
        },
    })
}
/// Untrusted claims confer no authority. Complete deterministic recomputation
/// must match before this function can return the private verified type.
pub fn verify_computation_claim(
    claim: &NumericCertificateClaim,
    graph: &NumericComputation,
    requirement: &NumericRequirement,
) -> Result<VerifiedNumericWitness, NumericEngineRefusal> {
    let verified = certify_computation_for_algorithm(graph, requirement, claim.algorithm)?;
    if verified.claim() != claim {
        return Err(NumericEngineRefusal::ClaimMismatch);
    }
    Ok(verified)
}

pub fn verify_encoded_computation_claim(
    bytes: &[u8],
    graph: &NumericComputation,
    requirement: &NumericRequirement,
) -> Result<VerifiedNumericWitness, NumericEngineRefusal> {
    let claim = NumericCertificateClaim::decode(bytes)?;
    verify_computation_claim(&claim, graph, requirement)
}

/// Candidate selection is external. Automatic preserves the original consumer
/// requirement identity while this engine proves the explicitly selected strategy.
pub fn certify_computation_for_algorithm(
    graph: &NumericComputation,
    original: &NumericRequirement,
    algorithm: NumericAlgorithm,
) -> Result<VerifiedNumericWitness, NumericEngineRefusal> {
    original.validate()?;
    let representation = match algorithm {
        NumericAlgorithm::OrderedCheckedI64V1 => NumericRepresentation::CheckedI64,
        NumericAlgorithm::OrderedRationalV1 | NumericAlgorithm::ExactComparisonV1 => {
            NumericRepresentation::ExactRational
        }
        NumericAlgorithm::OrderedSoftwareBinary64V1 => NumericRepresentation::Binary64,
        NumericAlgorithm::OrderedIeeeSoftwareBinary64V1 => NumericRepresentation::IeeeBinary64,
        NumericAlgorithm::CertifiedRationalIntervalV1 => NumericRepresentation::CertifiedInterval,
        NumericAlgorithm::SymbolicRetentionV1 => NumericRepresentation::Symbolic,
    };
    if original.representation != NumericRepresentation::Automatic
        && original.representation != representation
    {
        return Err(NumericEngineRefusal::UnsupportedRepresentation);
    }
    let mut selected = original.clone();
    selected.representation = representation;
    let mut witness = certify_computation(graph, &selected)?;
    if witness.claim.algorithm != algorithm {
        return Err(NumericEngineRefusal::UnsupportedRepresentation);
    }
    witness.claim.requirement_identity = original.identity()?;
    witness.requirement = original.clone();
    Ok(witness)
}
