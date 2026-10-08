//! Pre-publication quotas over every requested preparation allocation.
use super::*;
use core::mem::size_of;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedExpressionStorageRefusal {
    Capacity,
    UnsupportedTemporaryOperation,
    Expression(Refusal),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedExpressionStorageReceipt {
    pub decoded_program_heap_bytes: usize,
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
}
/// Charges accumulate without refunds. Thus the charged sum bounds the peak,
/// even when temporary encodings and already-built siblings overlap.
pub(super) struct PreparationBudget {
    maximum: Option<usize>,
    pub(super) charged: usize,
    pub(super) exhausted: bool,
    pub(super) unsupported: bool,
}
impl PreparationBudget {
    fn unlimited() -> Self {
        Self {
            maximum: None,
            charged: 0,
            exhausted: false,
            unsupported: false,
        }
    }
    pub(super) fn reserve(&mut self, bytes: usize) -> Result<(), Refusal> {
        if let Some(maximum) = self.maximum {
            let next = self.charged.checked_add(bytes);
            if next.is_none_or(|next| next > maximum) {
                self.exhausted = true;
                return Err(Refusal::InvalidProgram);
            }
            self.charged = next.unwrap();
        }
        Ok(())
    }
    pub(super) fn array<T>(&mut self, count: usize) -> Result<(), Refusal> {
        if self.maximum.is_none() {
            return Ok(());
        }
        let bytes = count.checked_mul(size_of::<T>()).ok_or_else(|| {
            self.exhausted = true;
            Refusal::InvalidProgram
        })?;
        self.reserve(bytes)
    }
    pub(super) fn vector<T>(&mut self, count: usize) -> Result<(), Refusal> {
        if self.maximum.is_none() {
            return Ok(());
        }
        // Result collection may grow geometrically rather than reserve its exact
        // hint. A nonempty vector's final capacity is at most twice max(4,count); also reserve its previous buffer for
        // a moving realloc.
        let count = count.max(4).checked_mul(3).ok_or_else(|| {
            self.exhausted = true;
            Refusal::InvalidProgram
        })?;
        self.array::<T>(count)
    }
    pub(super) fn prefix(&mut self, ty: &conduit_core::StructuredInfoType) -> Result<(), Refusal> {
        if self.maximum.is_none() {
            return Ok(());
        }
        self.reserve(
            ty.canonical_byte_length()
                .map_err(|_| Refusal::InvalidProgram)?,
        )
    }
    pub(super) fn owned_type(
        &mut self,
        ty: &conduit_core::StructuredInfoType,
    ) -> Result<(), Refusal> {
        if self.maximum.is_none() {
            return Ok(());
        }
        self.reserve(ty.owned_heap_bytes())
    }
    pub(super) fn is_bounded(&self) -> bool {
        self.maximum.is_some()
    }
    pub(super) fn literal(
        &mut self,
        ty: &conduit_core::StructuredInfoType,
        text: &str,
    ) -> Result<(), Refusal> {
        if self.maximum.is_none() {
            return Ok(());
        }
        // Primitive literal preparation retains one encoded vector, briefly
        // owns/clones the conformance Value and nominal framing, and may parse
        // quoted text or canonicalize an integer. Reserve every such owner.
        for _ in 0..4 {
            self.owned_type(ty)?;
            self.prefix(ty)?;
            self.reserve(super::storage_bound::output(ty)?)?;
            self.reserve(text.len())?;
        }
        self.reserve(256) // fixed integer/unit/quantity formatting and identifiers
    }
    pub(super) fn refuse_structured_literal(&mut self) -> Result<(), Refusal> {
        if self.maximum.is_some() {
            self.unsupported = true;
            Err(Refusal::InvalidProgram)
        } else {
            Ok(())
        }
    }
}
impl PreparedPortableExpressionEvaluator {
    pub fn new(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        Self::new_in_budget(program, &mut PreparationBudget::unlimited())
    }
    /// Admission counts caller-owned decoded metadata separately from new
    /// preparation storage. Every quota is checked before evaluator publication.
    /// Structured literal materialization currently refuses this bounded entrance.
    pub fn new_with_storage_limits(
        program: &PortableExpressionProgram,
        maximum_decoded_program_bytes: usize,
        maximum_preparation_peak_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<(Self, PreparedExpressionStorageReceipt), PreparedExpressionStorageRefusal> {
        let mut remaining = conduit_core::MAXIMUM_STRUCTURED_INFO_NODES;
        check_node_bounds(&program.root, 0, &mut remaining)
            .map_err(PreparedExpressionStorageRefusal::Expression)?;
        let decoded = program.owned_heap_bytes();
        if decoded == usize::MAX || decoded > maximum_decoded_program_bytes {
            return Err(PreparedExpressionStorageRefusal::Capacity);
        }
        let maximum = maximum_preparation_peak_bytes
            .checked_sub(decoded)
            .ok_or(PreparedExpressionStorageRefusal::Capacity)?;
        let mut budget = PreparationBudget {
            maximum: Some(maximum),
            charged: 0,
            exhausted: false,
            unsupported: false,
        };
        let prepared = Self::new_in_budget(program, &mut budget).map_err(|error| {
            if budget.exhausted {
                PreparedExpressionStorageRefusal::Capacity
            } else if budget.unsupported {
                PreparedExpressionStorageRefusal::UnsupportedTemporaryOperation
            } else {
                PreparedExpressionStorageRefusal::Expression(error)
            }
        })?;
        let retained = prepared.owned_heap_bytes();
        if retained == usize::MAX || retained > maximum_retained_bytes {
            return Err(PreparedExpressionStorageRefusal::Capacity);
        }
        Ok((
            prepared,
            PreparedExpressionStorageReceipt {
                decoded_program_heap_bytes: decoded,
                preparation_requested_bytes_bound: budget.charged,
                retained_heap_bytes_bound: retained,
            },
        ))
    }
    fn new_in_budget(
        program: &PortableExpressionProgram,
        budget: &mut PreparationBudget,
    ) -> Result<Self, Refusal> {
        budget.owned_type(&program.input_type)?; // allocated kind refusal
        let input = match program.input_type.shape() {
            StructuredInfoTypeShape::Leaf(_) => PreparedInput::Primitive {
                kind: leaf_kind(&program.input_type)?,
                nominal_type: None,
            },
            StructuredInfoTypeShape::Nominal { representation, .. }
                if matches!(representation.shape(), StructuredInfoTypeShape::Leaf(_)) =>
            {
                budget.prefix(&program.input_type)?;
                budget.reserve(
                    program
                        .input_type
                        .canonical_byte_length()
                        .map_err(|_| Refusal::InvalidProgram)?
                        .saturating_add(2 * size_of::<usize>())
                        .saturating_add(core::mem::align_of::<usize>() - 1),
                )?;
                PreparedInput::Primitive {
                    kind: leaf_kind(&program.input_type)?,
                    nominal_type: Some(
                        program
                            .input_type
                            .canonical_bytes()
                            .map_err(|_| Refusal::InvalidProgram)?
                            .into(),
                    ),
                }
            }
            _ => {
                budget.prefix(&program.input_type)?;
                budget.reserve(
                    program
                        .input_type
                        .canonical_byte_length()
                        .map_err(|_| Refusal::InvalidProgram)?
                        .saturating_add(2 * size_of::<usize>())
                        .saturating_add(core::mem::align_of::<usize>() - 1),
                )?;
                PreparedInput::Structured(
                    program
                        .input_type
                        .canonical_bytes()
                        .map_err(|_| Refusal::InvalidProgram)?
                        .into(),
                )
            }
        };
        Self::prepare(
            ProgramView {
                input_type: &program.input_type,
                output_type: &program.output_type,
                root: &program.root,
            },
            input,
            budget,
        )
    }
}

// Match the canonical decoder's finite AST envelope before recursive accounting.
fn check_node_bounds(
    node: &PortableExpressionNode,
    depth: usize,
    remaining: &mut usize,
) -> Result<(), Refusal> {
    if depth > conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH {
        return Err(Refusal::InvalidProgram);
    }
    *remaining = remaining.checked_sub(1).ok_or(Refusal::InvalidProgram)?;
    let mut child = |node: &PortableExpressionNode| check_node_bounds(node, depth + 1, remaining);
    match &node.operation {
        PortableExpressionOperation::Input | PortableExpressionOperation::Literal(_) => Ok(()),
        PortableExpressionOperation::Projection { value, .. } => child(value),
        PortableExpressionOperation::Unary { operand, .. } => child(operand),
        PortableExpressionOperation::Variant { payload, .. } => child(payload),
        PortableExpressionOperation::Binary { left, right, .. } => {
            child(left)?;
            child(right)
        }
        PortableExpressionOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            child(condition)?;
            child(when_true)?;
            child(when_false)
        }
        PortableExpressionOperation::Tuple(nodes)
        | PortableExpressionOperation::Collection(nodes)
        | PortableExpressionOperation::SemanticCall {
            arguments: nodes, ..
        } => {
            for node in nodes {
                child(node)?;
            }
            Ok(())
        }
        PortableExpressionOperation::Record(fields) => {
            for (_, node) in fields {
                child(node)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cumulative_quota_refuses_checked_overflow_before_allocation() {
        let mut budget = PreparationBudget {
            maximum: Some(usize::MAX),
            charged: 0,
            exhausted: false,
            unsupported: false,
        };
        budget.reserve(usize::MAX).unwrap();
        assert_eq!(budget.reserve(1), Err(Refusal::InvalidProgram));
        assert!(budget.exhausted);
    }
}
