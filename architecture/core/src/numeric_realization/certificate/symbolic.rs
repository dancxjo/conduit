//! Exact symbolic retention is a finite representation, not a floating estimate.
use super::*;
pub(super) fn certify_symbolic(
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
    let mut storage = crate::MAX_SYMBOLIC_NODES * core::mem::size_of::<crate::ExactRational>()
        + crate::MAX_NUMERIC_COMPUTATION_BYTES
        + 4 * crate::MAX_NUMERIC_ENCODED_BYTES;
    if storage > requirement.budget.storage_bytes as usize {
        return Err(NumericRefusal::StorageBudget.into());
    }
    let mut peak_storage = storage;
    let mut required_symbolic_nodes = 0;
    let mut values: Vec<crate::NumericComputedResult> = Vec::with_capacity(graph.nodes().len());
    for node in graph.nodes() {
        // Covers the finite ordered primitive calls in coordinate normalization,
        // signature/profile checks and one authoritative operation. Each call
        // also carries the Core fixed-limb/work bound.
        work.charge(64)?;
        if let NumericComputationOperation::Apply { operands, .. } = &node.operation {
            if operands
                .iter()
                .any(|index| matches!(&values[usize::from(*index)], crate::NumericComputedResult::Number(value) if matches!(value.value, MathValue::Symbolic(_))))
            {
                required_symbolic_nodes = crate::MAX_SYMBOLIC_NODES as u16;
                if required_symbolic_nodes > requirement.budget.symbolic_nodes {
                    return Err(NumericRefusal::NodeBudget.into());
                }
            }
        }
        let result = match &node.operation {
            NumericComputationOperation::IeeeLiteral { .. }
            | NumericComputationOperation::IeeeRoundedLiteral { .. } => {
                return Err(NumericEngineRefusal::UnsupportedRepresentation)
            }
            NumericComputationOperation::Literal(bytes) => {
                let decoded =
                    crate::MathValueRef::validate(bytes)?.owned_storage_upper_bound_bytes();
                if storage
                    .checked_add(decoded)
                    .ok_or(NumericRefusal::StorageBudget)?
                    > requirement.budget.storage_bytes as usize
                {
                    return Err(NumericRefusal::StorageBudget.into());
                }
                crate::NumericComputedResult::Number(crate::NumericComputedValue {
                    value: MathValue::decode(bytes)?,
                    semantic_kind: node.semantic_kind.clone(),
                    unit: node.unit.clone(),
                })
            }
            NumericComputationOperation::Parameter(slot) => {
                let range = requirement
                    .parameters
                    .iter()
                    .find(|range| range.slot == *slot)
                    .ok_or(NumericEngineRefusal::MissingParameter)?;
                if range.lower.compare(&range.upper)? != Some(Ordering::Equal) {
                    return Err(NumericEngineRefusal::UnsupportedRange);
                }
                if storage
                    .checked_add(range.lower.owned_storage_bytes())
                    .ok_or(NumericRefusal::StorageBudget)?
                    > requirement.budget.storage_bytes as usize
                {
                    return Err(NumericRefusal::StorageBudget.into());
                }
                crate::NumericComputedResult::Number(crate::NumericComputedValue {
                    value: range.lower.clone(),
                    semantic_kind: node.semantic_kind.clone(),
                    unit: node.unit.clone(),
                })
            }
            NumericComputationOperation::Apply { operands, .. } => {
                // The authoritative physical helper can normalize both inputs,
                // build an output coordinate and validate its captured laws. Reserve
                // thirty-two simultaneous owned copies of every input and captured law
                // before making any of those allocations. This is a conservative
                // implementation bound, not a bound inferred from compressed wire size.
                let mut clone_workspace = 0usize;
                for index in operands {
                    if let crate::NumericComputedResult::Number(value) =
                        &values[usize::from(*index)]
                    {
                        clone_workspace += value.value.owned_storage_bytes();
                        if let Some(unit) = &value.unit {
                            clone_workspace += unit.owned_storage_bytes()
                                + unit.exact_scale().owned_storage_upper_bound_bytes()
                                + unit.exact_point_offset().owned_storage_upper_bound_bytes();
                        }
                    }
                }
                for unit in [node.unit.as_ref(), node.reference_unit.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    clone_workspace += unit.owned_storage_bytes()
                        + unit.exact_scale().owned_storage_upper_bound_bytes()
                        + unit.exact_point_offset().owned_storage_upper_bound_bytes();
                }
                let operation_peak = storage
                    .checked_add(
                        clone_workspace
                            .checked_mul(32)
                            .ok_or(NumericRefusal::StorageBudget)?,
                    )
                    .ok_or(NumericRefusal::StorageBudget)?;
                if operation_peak > requirement.budget.storage_bytes as usize {
                    return Err(NumericRefusal::StorageBudget.into());
                }
                peak_storage = peak_storage.max(operation_peak);
                let arguments = operands
                    .iter()
                    .map(|index| match &values[usize::from(*index)] {
                        crate::NumericComputedResult::Number(value) => Ok(value.clone()),
                        _ => Err(NumericEngineRefusal::InvalidComputation),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                crate::apply_numeric_operation_in_node(node, &arguments).map_err(|error| {
                    match error {
                        crate::NumericEvaluationRefusal::Arithmetic(error) => {
                            NumericEngineRefusal::Arithmetic(error)
                        }
                        _ => NumericEngineRefusal::UnsupportedPhysicalLaw,
                    }
                })?
            }
        };
        let value = match &result {
            crate::NumericComputedResult::Number(value) => {
                if is_math_kind(&node.semantic_kind) {
                    admit_math_value(&node.semantic_kind, &value.value)?;
                }
                if let Some(unit) = &value.unit {
                    let (_, role) = crate::parse_quantity_semantic_role(&node.semantic_kind)
                        .ok_or(NumericEngineRefusal::UnsupportedPhysicalLaw)?;
                    let quantity = crate::Quantity::from_math_role(value.value.clone(), unit, role)
                        .map_err(|_| NumericEngineRefusal::UnsupportedPhysicalLaw)?;
                    crate::validate_primitive_info(&node.semantic_kind, quantity.encoded())
                        .map_err(|_| NumericEngineRefusal::UnsupportedPhysicalLaw)?;
                }
                if value.unit != node.unit {
                    return Err(NumericEngineRefusal::UnsupportedPhysicalLaw);
                }
                &value.value
            }
            crate::NumericComputedResult::Boolean(_) => {
                if node.semantic_kind != crate::BOOL_INFO_ID {
                    return Err(NumericEngineRefusal::InvalidComputation);
                }
                values.push(result);
                continue;
            }
        };
        if let MathValue::Symbolic(dag) = value {
            required_symbolic_nodes = required_symbolic_nodes.max(dag.node_count() as u16);
            if dag.node_count() > usize::from(requirement.budget.symbolic_nodes) {
                return Err(NumericRefusal::NodeBudget.into());
            }
        }
        // The accessor includes actual retained rational boxes/DAG buffers.
        storage += value.owned_storage_bytes();
        if storage > requirement.budget.storage_bytes as usize {
            return Err(NumericRefusal::StorageBudget.into());
        }
        if let crate::NumericComputedResult::Number(value) = &result {
            storage += value.unit.as_ref().map_or(0, |unit| unit.encoded().len());
        }
        peak_storage = peak_storage.max(storage);
        if peak_storage > requirement.budget.storage_bytes as usize {
            return Err(NumericRefusal::StorageBudget.into());
        }
        values.push(result);
    }
    let result = match values[usize::from(graph.root())].clone() {
        crate::NumericComputedResult::Boolean(value) => NumericResultEvidence::ExactBoolean(value),
        crate::NumericComputedResult::Number(value) => match value.unit {
            None => NumericResultEvidence::ExactMathematical(value.value),
            Some(unit) => {
                let (_, role) = crate::parse_quantity_semantic_role(&value.semantic_kind)
                    .ok_or(NumericEngineRefusal::UnsupportedPhysicalLaw)?;
                let quantity = crate::Quantity::from_math_role(value.value, &unit, role)
                    .map_err(|_| NumericEngineRefusal::UnsupportedPhysicalLaw)?;
                NumericResultEvidence::ExactPhysical(alloc::boxed::Box::new(quantity))
            }
        },
    };
    match &requirement.accuracy {
        NumericAccuracy::Exact
        | NumericAccuracy::Absolute(_)
        | NumericAccuracy::Relative { .. } => {}
        _ => return Err(NumericEngineRefusal::AccuracyUnproved),
    }
    Ok(VerifiedNumericWitness {
        requirement: requirement.clone(),
        claim: NumericCertificateClaim {
            engine_identity: crate::numeric_realization::numeric_engine_identity(),
            graph_identity: graph
                .identity()
                .map_err(|_| NumericEngineRefusal::InvalidComputation)?,
            requirement_identity: requirement.identity()?,
            algorithm: NumericAlgorithm::SymbolicRetentionV1,
            result,
            error: RationalInterval::point(MathValue::integer(0))?,
            steps: Vec::new(),
            boolean_steps: Vec::new(),
            integer_steps: Vec::new(),
            flags: Binary64Flags::default(),
            work_steps: work.used(),
            storage_bytes: peak_storage as u32,
            required_limb_capacity: crate::MAX_INTEGER_LIMBS as u16,
            required_symbolic_nodes,
        },
    })
}
