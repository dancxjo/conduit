//! Prepared allocation-free typed variant case discrimination.

use super::{PrimitiveValue, Refusal};
use crate::{PortableExpressionNode, PortableExpressionOperation, PortableExpressionProjection};
use alloc::{string::String, vec::Vec};
use conduit_core::{
    InfoBool, PrimitiveInfoKind, StructuredCanonicalSelection, StructuredInfoType,
    StructuredInfoTypeShape, StructuredSelector, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub(super) struct PreparedVariantCaseTest {
    steps: Vec<Step>,
    variant_type: Vec<u8>,
    expected: String,
    first: Vec<u8>,
    second: Vec<u8>,
}

struct Step {
    selector: StructuredSelector,
    input_type: Vec<u8>,
    output_type: Vec<u8>,
}

impl PreparedVariantCaseTest {
    pub(super) fn new(value: &PortableExpressionNode, expected: &str) -> Result<Self, Refusal> {
        fn collect(
            value: &PortableExpressionNode,
            steps: &mut Vec<Step>,
        ) -> Result<StructuredInfoType, Refusal> {
            match &value.operation {
                PortableExpressionOperation::Input => Ok(value.value_type.clone()),
                PortableExpressionOperation::Projection {
                    value: parent,
                    member,
                } => {
                    let input_type = collect(parent, steps)?;
                    let selector = match member {
                        PortableExpressionProjection::Field(field) => {
                            StructuredSelector::field(input_type.clone(), field.clone())
                        }
                        PortableExpressionProjection::TupleIndex(index) => {
                            StructuredSelector::field(
                                input_type.clone(),
                                alloc::format!("item-{index:05}"),
                            )
                        }
                    }
                    .map_err(|_| Refusal::InvalidProgram)?;
                    if selector.output_type() != &value.value_type {
                        return Err(Refusal::InvalidProgram);
                    }
                    steps.push(Step {
                        input_type: selector
                            .input_type()
                            .canonical_bytes()
                            .map_err(|_| Refusal::InvalidProgram)?,
                        output_type: selector
                            .output_type()
                            .canonical_bytes()
                            .map_err(|_| Refusal::InvalidProgram)?,
                        selector,
                    });
                    Ok(value.value_type.clone())
                }
                _ => Err(Refusal::UnsupportedType(
                    "case-test operand must be an input projection".into(),
                )),
            }
        }

        let mut steps = Vec::new();
        let value_type = collect(value, &mut steps)?;
        let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
            return Err(Refusal::InvalidProgram);
        };
        if !cases.iter().any(|case| case.tag() == expected) {
            return Err(Refusal::InvalidProgram);
        }
        Ok(Self {
            steps,
            variant_type: value_type
                .canonical_bytes()
                .map_err(|_| Refusal::InvalidProgram)?,
            expected: expected.into(),
            first: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            second: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    pub(super) fn evaluate(&mut self, input: &[u8]) -> Result<PrimitiveValue, Refusal> {
        self.first.clear();
        self.first.extend_from_slice(input);
        for step in &self.steps {
            self.second.clear();
            let selection = step
                .selector
                .select_canonical_into(
                    &self.first,
                    &step.input_type,
                    &step.output_type,
                    &mut self.second,
                )
                .map_err(|_| Refusal::InvalidInput)?;
            if selection != StructuredCanonicalSelection::Matched {
                return Err(Refusal::InvalidInput);
            }
            core::mem::swap(&mut self.first, &mut self.second);
        }
        let node = self
            .first
            .strip_prefix(self.variant_type.as_slice())
            .ok_or(Refusal::InvalidInput)?;
        let [3, rest @ ..] = node else {
            return Err(Refusal::InvalidInput);
        };
        if rest.len() < 4 {
            return Err(Refusal::InvalidInput);
        }
        let length = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
        let tag = rest.get(4..4 + length).ok_or(Refusal::InvalidInput)?;
        PrimitiveValue::new(
            PrimitiveInfoKind::Bool,
            &InfoBool::new(tag == self.expected.as_bytes()).encode(),
        )
    }
}
