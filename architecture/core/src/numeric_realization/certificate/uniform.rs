use super::*;

pub(super) fn certify_interval(
    graph: &NumericComputation,
    requirement: &NumericRequirement,
) -> Result<VerifiedNumericWitness, NumericEngineRefusal> {
    if requirement.semantics != NumericSemantics::Mathematical {
        return Err(NumericEngineRefusal::UnsupportedRepresentation);
    }
    if usize::from(requirement.budget.limbs) != crate::MAX_INTEGER_LIMBS {
        return Err(NumericRefusal::IntegerBudget.into());
    }
    let mut work = NumericRealizationWork::new(requirement.budget.work_steps);
    // Reserve retained endpoint storage and bounded nested enclosure/scratch
    // before executing the first numerical operation.
    let storage =
        (32 + graph.nodes().len() * 2 + usize::from(requirement.budget.symbolic_nodes) * 2)
            * core::mem::size_of::<crate::ExactRational>()
            + graph.nodes().len() * core::mem::size_of::<RationalInterval>()
            + crate::MAX_NUMERIC_COMPUTATION_BYTES
            + crate::MAX_NUMERIC_ENCODED_BYTES;
    if storage > requirement.budget.storage_bytes as usize {
        return Err(NumericRefusal::StorageBudget.into());
    }
    let mut required_symbolic_nodes = 0;
    let mut values: Vec<RationalInterval> = Vec::with_capacity(graph.nodes().len());
    for node in graph.nodes() {
        work.charge(4)?;
        if node.unit.is_some()
            || node.reference_unit.is_some()
            || !is_math_kind(&node.semantic_kind)
        {
            return Err(NumericEngineRefusal::UnsupportedPhysicalLaw);
        }
        let value = match &node.operation {
            NumericComputationOperation::IeeeLiteral { .. }
            | NumericComputationOperation::IeeeRoundedLiteral { .. } => {
                return Err(NumericEngineRefusal::UnsupportedRepresentation)
            }
            NumericComputationOperation::Literal(bytes) => {
                let literal = MathValue::decode(bytes)?;
                if let MathValue::Symbolic(dag) = &literal {
                    required_symbolic_nodes = required_symbolic_nodes.max(dag.node_count() as u16);
                    if dag.node_count() > usize::from(requirement.budget.symbolic_nodes) {
                        return Err(NumericRefusal::NodeBudget.into());
                    }
                }
                crate::numeric_realization::enclose_math_value(&literal, &mut work)?
            }
            NumericComputationOperation::Parameter(slot) => {
                let range = requirement
                    .parameters
                    .iter()
                    .find(|range| range.slot == *slot)
                    .ok_or(NumericEngineRefusal::MissingParameter)?;
                RationalInterval::new(range.lower.clone(), range.upper.clone())?
            }
            NumericComputationOperation::Apply {
                operation,
                operands,
            } => {
                let operand = |index| &values[usize::from(operands[index])];
                match operation {
                    NumericOperation::Negate => operand(0).negate()?,
                    NumericOperation::Add => operand(0).add(operand(1))?,
                    NumericOperation::Subtract => operand(0).subtract(operand(1))?,
                    NumericOperation::Multiply => operand(0).multiply(operand(1))?,
                    NumericOperation::Divide => operand(0).divide(operand(1))?,
                    NumericOperation::SquareRoot => RationalInterval::new(
                        crate::numeric_realization::enclose_sqrt(
                            operand(0).lower(),
                            80,
                            &mut work,
                        )?
                        .lower()
                        .clone(),
                        crate::numeric_realization::enclose_sqrt(
                            operand(0).upper(),
                            80,
                            &mut work,
                        )?
                        .upper()
                        .clone(),
                    )?,
                    _ => return Err(NumericEngineRefusal::UnsupportedOperation(*operation)),
                }
            }
        };
        values.push(value);
    }
    let result = values[usize::from(graph.root())].clone();
    let limit = match &requirement.accuracy {
        NumericAccuracy::EnclosureWidth(limit) => limit.clone(),
        NumericAccuracy::Exact => MathValue::integer(0),
        _ => return Err(NumericEngineRefusal::AccuracyUnproved),
    };
    if !matches!(
        result.width()?.compare(&limit)?,
        Some(Ordering::Less | Ordering::Equal)
    ) {
        return Err(NumericEngineRefusal::AccuracyUnproved);
    }
    Ok(VerifiedNumericWitness {
        requirement: requirement.clone(),
        claim: NumericCertificateClaim {
            engine_identity: crate::numeric_realization::numeric_engine_identity(),
            graph_identity: graph
                .identity()
                .map_err(|_| NumericEngineRefusal::InvalidComputation)?,
            algorithm: NumericAlgorithm::CertifiedRationalIntervalV1,
            requirement_identity: requirement.identity()?,
            result: NumericResultEvidence::Enclosure(result),
            error: RationalInterval::point(MathValue::integer(0))?,
            steps: Vec::new(),
            boolean_steps: Vec::new(),
            integer_steps: Vec::new(),
            flags: Binary64Flags::default(),
            work_steps: work.used(),
            storage_bytes: storage as u32,
            required_limb_capacity: crate::MAX_INTEGER_LIMBS as u16,
            required_symbolic_nodes,
        },
    })
}
