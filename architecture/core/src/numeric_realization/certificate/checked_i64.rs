//! Exact native integer strategy: each ordered operation checks overflow/inexactness.
use super::*;
fn integer(value: &MathValue) -> Result<i64, NumericEngineRefusal> {
    match value {
        MathValue::SmallInteger(value) => Ok(*value),
        _ => {
            let bytes = value.canonical_bytes()?;
            crate::MathValueRef::validate(&bytes)?
                .checked_i64()
                .map_err(projection_refusal)
        }
    }
}
fn projection_refusal(value: crate::NumericProjectionRefusal) -> NumericEngineRefusal {
    match value {
        crate::NumericProjectionRefusal::Inexact
        | crate::NumericProjectionRefusal::UnresolvedSymbolic => {
            NumericEngineRefusal::InexactInteger
        }
        crate::NumericProjectionRefusal::Overflow => NumericEngineRefusal::IntegerOverflow,
        crate::NumericProjectionRefusal::Budget(value) => value.into(),
    }
}
pub(super) fn certify_i64(
    graph: &NumericComputation,
    requirement: &NumericRequirement,
) -> Result<VerifiedNumericWitness, NumericEngineRefusal> {
    if requirement.semantics != NumericSemantics::Mathematical {
        return Err(NumericEngineRefusal::UnsupportedRepresentation);
    }
    let storage = 8 * core::mem::size_of::<crate::ExactRational>()
        + graph
            .canonical_bytes()
            .map_err(|_| NumericEngineRefusal::InvalidComputation)?
            .len()
        + requirement.canonical_bytes()?.len()
        + 512
        + graph.nodes().len() * (core::mem::size_of::<I64StepEvidence>() + 24);
    if storage > requirement.budget.storage_bytes as usize {
        return Err(NumericRefusal::StorageBudget.into());
    }
    let mut work = NumericRealizationWork::new(requirement.budget.work_steps);
    let mut values: Vec<i64> = Vec::with_capacity(graph.nodes().len());
    let mut steps = Vec::with_capacity(graph.nodes().len());
    let mut boolean = None;
    for (index, node) in graph.nodes().iter().enumerate() {
        work.charge(1)?;
        if node.unit.is_some() || node.reference_unit.is_some() {
            return Err(NumericEngineRefusal::UnsupportedPhysicalLaw);
        }
        let is_boolean =
            index == usize::from(graph.root()) && node.semantic_kind == crate::BOOL_INFO_ID;
        if !is_math_kind(&node.semantic_kind) && !is_boolean {
            return Err(NumericEngineRefusal::UnsupportedRepresentation);
        }
        let mut inputs = Vec::new();
        let value = match &node.operation {
            NumericComputationOperation::IeeeLiteral { .. }
            | NumericComputationOperation::IeeeRoundedLiteral { .. } => {
                return Err(NumericEngineRefusal::UnsupportedRepresentation)
            }
            NumericComputationOperation::Literal(bytes) => crate::MathValueRef::validate(bytes)?
                .checked_i64()
                .map_err(projection_refusal)?,
            NumericComputationOperation::Parameter(slot) => {
                let range = requirement
                    .parameters
                    .iter()
                    .find(|range| range.slot == *slot)
                    .ok_or(NumericEngineRefusal::MissingParameter)?;
                if range.lower.compare(&range.upper)? != Some(Ordering::Equal) {
                    return Err(NumericEngineRefusal::UnsupportedRange);
                }
                integer(&range.lower)?
            }
            NumericComputationOperation::Apply {
                operation,
                operands,
            } => {
                inputs = operands
                    .iter()
                    .map(|index| values[usize::from(*index)])
                    .collect();
                let a = inputs[0];
                let b = inputs.get(1).copied();
                if is_boolean {
                    let order = a.cmp(&b.ok_or(NumericEngineRefusal::InvalidComputation)?);
                    boolean = Some(match operation {
                        NumericOperation::Equal => order == Ordering::Equal,
                        NumericOperation::NotEqual => order != Ordering::Equal,
                        NumericOperation::Less => order == Ordering::Less,
                        NumericOperation::LessOrEqual => order != Ordering::Greater,
                        NumericOperation::Greater => order == Ordering::Greater,
                        NumericOperation::GreaterOrEqual => order != Ordering::Less,
                        _ => return Err(NumericEngineRefusal::UnsupportedOperation(*operation)),
                    });
                    a
                } else {
                    let value = match operation {
                        NumericOperation::Negate => a.checked_neg(),
                        NumericOperation::Add => {
                            a.checked_add(b.ok_or(NumericEngineRefusal::InvalidComputation)?)
                        }
                        NumericOperation::Subtract => {
                            a.checked_sub(b.ok_or(NumericEngineRefusal::InvalidComputation)?)
                        }
                        NumericOperation::Multiply => {
                            a.checked_mul(b.ok_or(NumericEngineRefusal::InvalidComputation)?)
                        }
                        NumericOperation::Divide => {
                            let b = b.ok_or(NumericEngineRefusal::InvalidComputation)?;
                            if b == 0 {
                                return Err(NumericRefusal::DivisionByZero.into());
                            }
                            let quotient = a
                                .checked_div(b)
                                .ok_or(NumericEngineRefusal::IntegerOverflow)?;
                            if a.checked_rem(b) != Some(0) {
                                return Err(NumericEngineRefusal::InexactInteger);
                            }
                            Some(quotient)
                        }
                        _ => return Err(NumericEngineRefusal::UnsupportedOperation(*operation)),
                    };
                    value.ok_or(NumericEngineRefusal::IntegerOverflow)?
                }
            }
        };
        if !is_boolean {
            admit_math_value(&node.semantic_kind, &MathValue::integer(value))?;
            steps.push(I64StepEvidence {
                node: index as u16,
                operands: inputs,
                output: value,
            });
        }
        values.push(value);
    }
    let result = match boolean {
        Some(value) => {
            if requirement.accuracy != NumericAccuracy::Exact {
                return Err(NumericEngineRefusal::AccuracyUnproved);
            }
            NumericResultEvidence::ExactBoolean(value)
        }
        None => {
            let exact = MathValue::integer(values[usize::from(graph.root())]);
            prove_accuracy(&exact, &MathValue::integer(0), &requirement.accuracy)?;
            NumericResultEvidence::ExactMathematical(exact)
        }
    };
    Ok(VerifiedNumericWitness {
        requirement: requirement.clone(),
        claim: NumericCertificateClaim {
            engine_identity: crate::numeric_realization::numeric_engine_identity(),
            graph_identity: graph
                .identity()
                .map_err(|_| NumericEngineRefusal::InvalidComputation)?,
            requirement_identity: requirement.identity()?,
            algorithm: NumericAlgorithm::OrderedCheckedI64V1,
            result,
            error: RationalInterval::point(MathValue::integer(0))?,
            steps: Vec::new(),
            boolean_steps: Vec::new(),
            integer_steps: steps,
            flags: Binary64Flags::default(),
            work_steps: work.used(),
            storage_bytes: storage as u32,
            required_limb_capacity: 1,
            required_symbolic_nodes: 0,
        },
    })
}
