use super::*;

pub(super) fn certify_exact_boolean(
    graph: &NumericComputation,
    requirement: &NumericRequirement,
) -> Result<VerifiedNumericWitness, NumericEngineRefusal> {
    if requirement.semantics != NumericSemantics::Mathematical
        || requirement.representation != NumericRepresentation::ExactRational
    {
        return Err(NumericEngineRefusal::UnsupportedRepresentation);
    }
    if usize::from(requirement.budget.limbs) != crate::MAX_INTEGER_LIMBS {
        return Err(NumericRefusal::IntegerBudget.into());
    }
    // Boolean truth cannot be licensed by an approximate tolerance.
    if requirement.accuracy != NumericAccuracy::Exact {
        return Err(NumericEngineRefusal::AccuracyUnproved);
    }
    let storage = (32 + graph.nodes().len()) * core::mem::size_of::<crate::ExactRational>()
        + graph.nodes().len() * core::mem::size_of::<MathValue>()
        + crate::MAX_NUMERIC_COMPUTATION_BYTES
        + crate::MAX_NUMERIC_ENCODED_BYTES;
    if storage > requirement.budget.storage_bytes as usize {
        return Err(NumericRefusal::StorageBudget.into());
    }
    let mut work = NumericRealizationWork::new(requirement.budget.work_steps);
    let mut values: Vec<MathValue> = Vec::with_capacity(graph.nodes().len());
    for (index, node) in graph.nodes().iter().enumerate() {
        work.charge(4)?;
        if node.unit.is_some() || node.reference_unit.is_some() {
            return Err(NumericEngineRefusal::UnsupportedPhysicalLaw);
        }
        if index == usize::from(graph.root()) {
            let NumericComputationOperation::Apply {
                operation,
                operands,
            } = &node.operation
            else {
                return Err(NumericEngineRefusal::InvalidComputation);
            };
            if operands.len() != 2 {
                return Err(NumericEngineRefusal::InvalidComputation);
            }
            let ordering = values[usize::from(operands[0])]
                .compare(&values[usize::from(operands[1])])?
                .ok_or(NumericEngineRefusal::AccuracyUnproved)?;
            let result = match operation {
                NumericOperation::Equal => ordering == Ordering::Equal,
                NumericOperation::NotEqual => ordering != Ordering::Equal,
                NumericOperation::Less => ordering == Ordering::Less,
                NumericOperation::LessOrEqual => ordering != Ordering::Greater,
                NumericOperation::Greater => ordering == Ordering::Greater,
                NumericOperation::GreaterOrEqual => ordering != Ordering::Less,
                _ => return Err(NumericEngineRefusal::UnsupportedOperation(*operation)),
            };
            return Ok(VerifiedNumericWitness {
                requirement: requirement.clone(),
                claim: NumericCertificateClaim {
                    engine_identity: crate::numeric_realization::numeric_engine_identity(),
                    graph_identity: graph
                        .identity()
                        .map_err(|_| NumericEngineRefusal::InvalidComputation)?,
                    algorithm: NumericAlgorithm::ExactComparisonV1,
                    requirement_identity: requirement.identity()?,
                    result: NumericResultEvidence::ExactBoolean(result),
                    error: RationalInterval::point(MathValue::integer(0))?,
                    steps: Vec::new(),
                    boolean_steps: Vec::new(),
                    integer_steps: Vec::new(),
                    flags: Binary64Flags::default(),
                    work_steps: work.used(),
                    storage_bytes: storage as u32,
                    required_limb_capacity: crate::MAX_INTEGER_LIMBS as u16,
                    required_symbolic_nodes: 0,
                },
            });
        }
        if !is_math_kind(&node.semantic_kind) {
            return Err(NumericEngineRefusal::UnsupportedPhysicalLaw);
        }
        let value = match &node.operation {
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
                    .map(|index| values[usize::from(*index)].clone())
                    .collect::<Vec<_>>(),
            )?,
        };
        if value.as_rational().is_none() {
            return Err(NumericEngineRefusal::AccuracyUnproved);
        }
        admit_math_value(&node.semantic_kind, &value)?;
        values.push(value);
    }
    Err(NumericEngineRefusal::InvalidComputation)
}
