//! Versioned finite input ceiling, independent of the structural Info shape.
use super::*;
use conduit_core::{CheckedValueContract, FrontValueContract, FrontValueLocation, KindSemanticLaw};

pub(super) const V2: &[u8] = b"conduit.pure-expression.program.v2";
pub(crate) const V3: &[u8] = b"conduit.pure-expression.program.v3";

pub(super) fn push_header(sink: &mut ProgramSink, bound: Option<u32>) {
    sink.extend_from_slice(if bound.is_some() { V3 } else { V2 });
    if let Some(maximum) = bound {
        sink.extend_from_slice(&maximum.to_le_bytes());
    }
}

impl PortableExpressionProgram {
    pub(super) fn encode(
        &self,
        encoded: &mut ProgramSink,
    ) -> Result<(), PortableExpressionProgramRefusal> {
        self.validate_input_bound()?;
        push_header(encoded, self.input_maximum_bytes);
        push_type(encoded, &self.input_type)?;
        push_type(encoded, &self.output_type)?;
        push_node(encoded, &self.root)?;
        if encoded.length > MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES {
            Err(PortableExpressionProgramRefusal::TooLarge)
        } else {
            Ok(())
        }
    }

    pub(crate) fn validate_input_bound(&self) -> Result<(), PortableExpressionProgramRefusal> {
        validate_bound(self.input_maximum_bytes)
    }

    pub fn semantic_laws(
        &self,
    ) -> Result<Vec<KindSemanticLaw>, conduit_core::StructuredInfoRefusal> {
        self.validate_input_bound()
            .map_err(|_| conduit_core::StructuredInfoRefusal::MalformedCanonicalEncoding)?;
        semantic_laws_for_input(
            crate::expression_definition::expression_port_kind(&self.input_type)?,
            self.input_maximum_bytes,
        )
    }
}

pub(crate) fn semantic_laws_for_input(
    input: conduit_core::KindId,
    bound: Option<u32>,
) -> Result<Vec<KindSemanticLaw>, conduit_core::StructuredInfoRefusal> {
    let mut laws = crate::pure_expression_semantic_laws();
    if let Some(maximum) = bound {
        laws.push(KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Input(conduit_core::port_id("input")),
            contract: CheckedValueContract::new(input, maximum, vec![])
                .map_err(|_| conduit_core::StructuredInfoRefusal::MalformedCanonicalEncoding)?,
        }]));
    }
    Ok(laws)
}

pub(super) fn validate_bound(bound: Option<u32>) -> Result<(), PortableExpressionProgramRefusal> {
    if bound
        .is_some_and(|maximum| maximum as usize > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES)
    {
        return Err(PortableExpressionProgramRefusal::MalformedEncoding);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
