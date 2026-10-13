//! Operational IEEE evidence retains raw bits and exception custody.
use super::*;
fn comparison(
    op: NumericOperation,
    a: Binary64Value,
    b: Binary64Value,
) -> Result<(bool, Binary64Flags), NumericEngineRefusal> {
    let relational = matches!(
        op,
        NumericOperation::Less
            | NumericOperation::LessOrEqual
            | NumericOperation::Greater
            | NumericOperation::GreaterOrEqual
    );
    if !relational && !matches!(op, NumericOperation::Equal | NumericOperation::NotEqual) {
        return Err(NumericEngineRefusal::UnsupportedOperation(op));
    }
    let nan = a.is_nan() || b.is_nan();
    let flags = Binary64Flags {
        invalid: if relational {
            nan
        } else {
            a.is_signaling_nan() || b.is_signaling_nan()
        },
        ..Default::default()
    };
    if nan {
        return Ok((op == NumericOperation::NotEqual, flags));
    }
    let key = |value: Binary64Value| {
        if value.is_negative() {
            !value.bits()
        } else {
            value.bits() | (1 << 63)
        }
    };
    let ordering = if a.is_zero() && b.is_zero() {
        Ordering::Equal
    } else {
        key(a).cmp(&key(b))
    };
    Ok((
        match op {
            NumericOperation::Equal => ordering == Ordering::Equal,
            NumericOperation::NotEqual => ordering != Ordering::Equal,
            NumericOperation::Less => ordering == Ordering::Less,
            NumericOperation::LessOrEqual => ordering != Ordering::Greater,
            NumericOperation::Greater => ordering == Ordering::Greater,
            NumericOperation::GreaterOrEqual => ordering != Ordering::Less,
            _ => return Err(NumericEngineRefusal::UnsupportedOperation(op)),
        },
        flags,
    ))
}
pub(super) fn certify_ieee(
    graph: &NumericComputation,
    requirement: &NumericRequirement,
) -> Result<VerifiedNumericWitness, NumericEngineRefusal> {
    if requirement.representation != NumericRepresentation::IeeeBinary64
        || requirement.accuracy != NumericAccuracy::Exact
    {
        return Err(NumericEngineRefusal::UnsupportedRepresentation);
    }
    if usize::from(requirement.budget.limbs) != crate::MAX_INTEGER_LIMBS {
        return Err(NumericRefusal::IntegerBudget.into());
    }
    let storage = 32 * core::mem::size_of::<crate::ExactRational>()
        + graph.nodes().len() * (core::mem::size_of::<Binary64StepEvidence>() + 24)
        + crate::MAX_NUMERIC_COMPUTATION_BYTES
        + crate::MAX_NUMERIC_ENCODED_BYTES;
    if storage > requirement.budget.storage_bytes as usize {
        return Err(NumericRefusal::StorageBudget.into());
    }
    let mut work = NumericRealizationWork::new(requirement.budget.work_steps);
    let mut values: Vec<Binary64Value> = Vec::with_capacity(graph.nodes().len());
    let mut steps = Vec::with_capacity(graph.nodes().len());
    let mut flags = Binary64Flags::default();
    let mut boolean = None;
    let mut boolean_steps = Vec::new();
    for (index, node) in graph.nodes().iter().enumerate() {
        work.charge(1)?;
        if node.unit.is_some() || node.reference_unit.is_some() {
            return Err(NumericEngineRefusal::UnsupportedPhysicalLaw);
        }
        if node.semantic_kind != crate::F64_INFO_ID
            && !(index == usize::from(graph.root()) && node.semantic_kind == crate::BOOL_INFO_ID)
        {
            return Err(NumericEngineRefusal::UnsupportedRepresentation);
        }
        let (value, node_flags, operands) = match &node.operation {
            NumericComputationOperation::IeeeLiteral { bits } => (
                Binary64Value::from_bits(*bits),
                Binary64Flags::default(),
                Vec::new(),
            ),
            NumericComputationOperation::IeeeRoundedLiteral {
                source,
                bits,
                flags,
            } => {
                let exact = MathValue::decode(source)?;
                let projected = Binary64Value::project(
                    &exact,
                    super::super::Binary64Rounding::NearestEven,
                    &mut work,
                )?;
                if projected.value.bits() != *bits || projected.flags != *flags {
                    return Err(NumericEngineRefusal::InvalidComputation);
                }
                (projected.value, projected.flags, Vec::new())
            }
            NumericComputationOperation::Parameter(_) => {
                return Err(NumericEngineRefusal::MissingParameter)
            }
            NumericComputationOperation::Literal(_) => {
                return Err(NumericEngineRefusal::UnsupportedRepresentation)
            }
            NumericComputationOperation::Apply {
                operation,
                operands,
            } => {
                let bits = operands
                    .iter()
                    .map(|index| values[usize::from(*index)].bits())
                    .collect();
                let a = values[usize::from(operands[0])];
                let b = operands.get(1).map(|index| values[usize::from(*index)]);
                if node.semantic_kind == crate::BOOL_INFO_ID {
                    let (result, flags) = comparison(
                        *operation,
                        a,
                        b.ok_or(NumericEngineRefusal::InvalidComputation)?,
                    )?;
                    boolean = Some(result);
                    boolean_steps.push(BooleanStepEvidence {
                        node: index as u16,
                        operands: operands
                            .iter()
                            .map(|index| values[usize::from(*index)].bits())
                            .collect(),
                        output: result,
                        flags,
                    });
                    // Boolean result is separately typed; no invented numeric bits.
                    (a, flags, bits)
                } else {
                    let result = binary_operation(*operation)?.apply(
                        a,
                        b,
                        requirement.rounding,
                        &mut work,
                    )?;
                    (result.value, result.flags, bits)
                }
            }
        };
        flags = flags.union(node_flags);
        if node.semantic_kind != crate::BOOL_INFO_ID {
            steps.push(Binary64StepEvidence {
                node: index as u16,
                operands,
                output: value.bits(),
                flags: node_flags,
            });
        }
        values.push(value);
    }
    if !requirement.exceptions.permits(flags) {
        return Err(NumericEngineRefusal::ExceptionPolicy);
    }
    let result = match boolean {
        Some(value) => NumericResultEvidence::ExactBoolean(value),
        None => NumericResultEvidence::IeeeBits(values[usize::from(graph.root())]),
    };
    Ok(VerifiedNumericWitness {
        requirement: requirement.clone(),
        claim: NumericCertificateClaim {
            engine_identity: crate::numeric_realization::numeric_engine_identity(),
            graph_identity: graph
                .identity()
                .map_err(|_| NumericEngineRefusal::InvalidComputation)?,
            algorithm: NumericAlgorithm::OrderedIeeeSoftwareBinary64V1,
            requirement_identity: requirement.identity()?,
            result,
            error: RationalInterval::point(MathValue::integer(0))?,
            steps,
            boolean_steps,
            integer_steps: Vec::new(),
            flags,
            work_steps: work.used(),
            storage_bytes: storage as u32,
            required_limb_capacity: crate::MAX_INTEGER_LIMBS as u16,
            required_symbolic_nodes: 0,
        },
    })
}
