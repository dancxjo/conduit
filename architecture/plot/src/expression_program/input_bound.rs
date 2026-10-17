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

    /// Filters emit unchanged bytes for Flow. Value filters wrap the payload
    /// in an optional value and retain the structural output ceiling.
    pub fn filter_semantic_laws(
        &self,
        temporal: conduit_core::PortTemporal,
    ) -> Result<Vec<KindSemanticLaw>, conduit_core::StructuredInfoRefusal> {
        self.validate_input_bound()
            .map_err(|_| conduit_core::StructuredInfoRefusal::MalformedCanonicalEncoding)?;
        let mut laws = semantic_laws_for_input(
            crate::expression_definition::expression_port_kind(&self.input_type)?,
            self.input_maximum_bytes,
        )?;
        if matches!(temporal, conduit_core::PortTemporal::Flow { .. }) {
            if let Some(maximum) = self.input_maximum_bytes {
                let contracts = laws
                    .iter_mut()
                    .find_map(|law| match law {
                        KindSemanticLaw::ValueContracts(contracts) => Some(contracts),
                        _ => None,
                    })
                    .expect("bounded input has one ValueContracts law");
                contracts.push(FrontValueContract {
                    location: FrontValueLocation::Output(conduit_core::port_id("output")),
                    contract: CheckedValueContract::new(
                        crate::expression_definition::expression_port_kind(&self.input_type)?,
                        maximum,
                        vec![],
                    )
                    .map_err(|_| conduit_core::StructuredInfoRefusal::MalformedCanonicalEncoding)?,
                });
            }
        }
        Ok(laws)
    }

    pub(crate) fn validate_input_bound(&self) -> Result<(), PortableExpressionProgramRefusal> {
        validate_bound(self.input_maximum_bytes)
    }

    pub fn semantic_laws(
        &self,
    ) -> Result<Vec<KindSemanticLaw>, conduit_core::StructuredInfoRefusal> {
        self.validate_input_bound()
            .map_err(|_| conduit_core::StructuredInfoRefusal::MalformedCanonicalEncoding)?;
        semantic_laws_for_identity(
            crate::expression_definition::expression_port_kind(&self.input_type)?,
            self.input_maximum_bytes,
            self.input_type == self.output_type
                && matches!(self.root.operation, PortableExpressionOperation::Input),
        )
    }
}

// An exact identity emits the admitted input bytes unchanged. Other
// expressions cannot inherit this bound, even when their shapes match.
pub(crate) fn semantic_laws_for_identity(
    input: conduit_core::KindId,
    bound: Option<u32>,
    identity: bool,
) -> Result<Vec<KindSemanticLaw>, conduit_core::StructuredInfoRefusal> {
    let mut laws = semantic_laws_for_input(input.clone(), bound)?;
    if let (true, Some(maximum)) = (identity, bound) {
        if let Some(KindSemanticLaw::ValueContracts(contracts)) = laws.last_mut() {
            contracts.push(FrontValueContract {
                location: FrontValueLocation::Output(conduit_core::port_id("output")),
                contract: CheckedValueContract::new(input, maximum, vec![])
                    .map_err(|_| conduit_core::StructuredInfoRefusal::MalformedCanonicalEncoding)?,
            });
        }
    }
    Ok(laws)
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
