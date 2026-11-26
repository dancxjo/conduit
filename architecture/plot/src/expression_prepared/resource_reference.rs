//! Opaque codec-owned resource references, never scalar or Text atoms.
use super::*;
use conduit_core::{BoundedResourceRef, StructuredInfoType, RESOURCE_REFERENCE_INFO_ID};

pub(super) fn is_reference(ty: &StructuredInfoType) -> bool {
    matches!(ty.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == RESOURCE_REFERENCE_INFO_ID)
}
pub(super) fn validate(bytes: &[u8]) -> Result<(), Refusal> {
    BoundedResourceRef::validate_encoded(bytes)
        .map(|_| ())
        .map_err(|_| Refusal::InvalidInput)
}
pub(super) struct PreparedReference {
    expression: PreparedStructuredExpression,
}
impl PreparedReference {
    pub(super) fn new(program: ProgramView<'_>, input: &PreparedInput) -> Result<Self, Refusal> {
        if !is_reference(program.output_type) {
            return Err(Refusal::InvalidProgram);
        }
        Ok(Self {
            expression: PreparedStructuredExpression::new(program, input)?,
        })
    }
    pub(super) fn evaluate(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), Refusal> {
        self.expression.evaluate(input, output)?;
        validate(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insufficient_prepared_output_refuses_instead_of_growing() {
        let ty = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
            conduit_core::RESOURCE_REFERENCE_INFO_ID,
        ))
        .unwrap();
        let program = PortableExpressionProgram {
            input_maximum_bytes: None,
            input_type: ty.clone(),
            output_type: ty.clone(),
            root: PortableExpressionNode {
                value_type: ty,
                operation: PortableExpressionOperation::Input,
            },
        };
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let reference = conduit_core::BoundedResourceRef {
            identity: conduit_core::ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: "value/text".into(),
            access_class: "resource/test".into(),
            extent: conduit_core::ResourceExtent {
                bytes: 1,
                items: Some(1),
            },
            lifetime: conduit_core::ResourceLifetime {
                version: conduit_core::ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        }
        .encode()
        .unwrap();
        evaluator.output = Vec::with_capacity(reference.len() - 1);
        let capacity = evaluator.output.capacity();
        assert!(evaluator.evaluate(&reference).is_err());
        assert_eq!(evaluator.output.capacity(), capacity);
    }
}
