//! Prepared, allocation-free observations of exact structured expression values.
use super::{
    primitive::PrimitiveValue, PreparedInput, PreparedPortableExpressionEvaluator, ProgramView,
    Refusal,
};
use crate::{PortableExpressionNode, PortableExpressionOperation};
use alloc::{boxed::Box, string::String};
use conduit_core::{PrimitiveInfoKind, StructuredInfoType, StructuredInfoTypeShape};

pub(super) struct PreparedInspection {
    source: Box<PreparedPortableExpressionEvaluator>,
    operation: Operation,
}
enum Operation {
    VariantIs(String),
    VariantTag,
    Length,
}
impl PreparedInspection {
    pub(super) fn new(
        call: &str,
        arguments: &[PortableExpressionNode],
        input: &StructuredInfoType,
        prepared_input: &PreparedInput,
    ) -> Result<Self, Refusal> {
        let source = arguments.first().ok_or(Refusal::InvalidProgram)?;
        let operation = match call {
            "variant/is" => {
                let [source, case] = arguments else {
                    return Err(Refusal::InvalidProgram);
                };
                let StructuredInfoTypeShape::Variant { cases, .. } = source.value_type.shape()
                else {
                    return Err(Refusal::InvalidProgram);
                };
                if case.value_type
                    != StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::TEXT_INFO_ID))
                        .map_err(|_| Refusal::InvalidProgram)?
                {
                    return Err(Refusal::InvalidProgram);
                }
                let PortableExpressionOperation::Literal(literal) = &case.operation else {
                    return Err(Refusal::InvalidProgram);
                };
                let case =
                    crate::text_value::parse_quoted_text(literal).ok_or(Refusal::InvalidProgram)?;
                if !cases.iter().any(|candidate| candidate.tag() == case) {
                    return Err(Refusal::InvalidProgram);
                }
                Operation::VariantIs(case)
            }
            "variant/tag"
                if arguments.len() == 1
                    && matches!(
                        source.value_type.shape(),
                        StructuredInfoTypeShape::Variant { .. }
                    ) =>
            {
                Operation::VariantTag
            }
            "sequence/length"
                if arguments.len() == 1
                    && matches!(
                        source.value_type.shape(),
                        StructuredInfoTypeShape::Collection { .. }
                            | StructuredInfoTypeShape::Sequence { .. }
                    ) =>
            {
                Operation::Length
            }
            _ => return Err(Refusal::InvalidProgram),
        };
        let program = ProgramView {
            input_type: input,
            output_type: &source.value_type,
            root: source,
        };
        Ok(Self {
            source: Box::new(PreparedPortableExpressionEvaluator::prepare(
                program,
                prepared_input.clone(),
            )?),
            operation,
        })
    }
    pub(super) fn evaluate(&mut self, input: &[u8]) -> Result<PrimitiveValue<'_>, Refusal> {
        let source = self.source.evaluate(input)?;
        let value = conduit_core::validate_canonical_structured_value(source)
            .map_err(|_| Refusal::InvalidInput)?;
        match &self.operation {
            Operation::VariantIs(wanted) => {
                let actual = value.variant_tag().map_err(|_| Refusal::InvalidInput)?;
                PrimitiveValue::new(PrimitiveInfoKind::Bool, &[u8::from(actual == wanted)])
            }
            Operation::VariantTag => PrimitiveValue::borrowed(
                PrimitiveInfoKind::Text,
                value
                    .variant_tag()
                    .map_err(|_| Refusal::InvalidInput)?
                    .as_bytes(),
            ),
            Operation::Length => PrimitiveValue::new(
                PrimitiveInfoKind::U64,
                &u64::from(
                    value
                        .collection_length()
                        .map_err(|_| Refusal::InvalidInput)?,
                )
                .to_le_bytes(),
            ),
        }
    }
}
