//! Existing evaluator execution and admitted output storage.
use super::*;
impl PreparedPortableExpressionEvaluator {
    pub fn evaluate(&mut self, input: &[u8]) -> Result<&[u8], Refusal> {
        let context = law_guard::context(&self.input).cloned();
        if let Some(context) = &context {
            context.ensure_live()?;
        }
        let result = self.evaluate_inner(input);
        if result.is_err() {
            if let Some(context) = context {
                context.fail();
            }
        }
        result
    }
    fn evaluate_inner(&mut self, input: &[u8]) -> Result<&[u8], Refusal> {
        if let Some(guard) = &mut self.input_law {
            guard.canonical(input)?;
        }
        if let Some(contract) = &self.input_value_contract {
            contract
                .validate(input)
                .map_err(Refusal::InputValueConstraint)?;
        }
        if self
            .input_maximum_bytes
            .is_some_and(|maximum| input.len() > maximum as usize)
        {
            return Err(Refusal::InvalidInput);
        }
        let mut primitive_bytes = input;
        let primitive_input = match &self.input {
            PreparedInput::Primitive {
                kind,
                info_kind,
                nominal_type,
                ..
            } => {
                if let Some(expected) = nominal_type {
                    primitive_bytes = nominal::input_payload(input, expected)?;
                }
                quantity_profile::validate_input(
                    info_kind,
                    primitive_bytes,
                    self.input.quantity_profile(),
                )?;
                Some(*kind)
            }
            PreparedInput::Structured(expected, _, _, _) => {
                let validated = conduit_core::validate_canonical_structured_value(input)
                    .map_err(|_| Refusal::InvalidInput)?;
                if validated.type_bytes() != expected.as_ref() {
                    return Err(Refusal::InvalidInput);
                }
                None
            }
        };
        self.output.clear();
        match &mut self.root {
            PreparedRoot::Primitive { node, nominal_type } => {
                let value = evaluate_node(node, primitive_bytes, primitive_input)?;
                if let Some(value_type) = nominal_type {
                    nominal::append_output(&mut self.output, value_type, &value)?;
                } else {
                    if value.length > self.output.capacity() {
                        return Err(Refusal::InvalidProgram);
                    }
                    self.output.extend_from_slice(value.as_slice());
                }
            }
            PreparedRoot::ScalarTape { tape, nominal_type } => {
                let value = tape.evaluate(primitive_bytes, primitive_input)?;
                if let Some(value_type) = nominal_type {
                    nominal::append_output(&mut self.output, value_type, &value)?;
                } else {
                    if value.length > self.output.capacity() {
                        return Err(Refusal::InvalidProgram);
                    }
                    self.output.extend_from_slice(value.as_slice());
                }
            }
            PreparedRoot::Structured(root) => root.evaluate(input, &mut self.output)?,
        }
        if let Some(contract) = &self.output_value_contract {
            contract
                .validate(&self.output)
                .map_err(Refusal::OutputValueConstraint)?;
        }
        if let Some(guard) = &mut self.output_law {
            guard.canonical(&self.output)?;
        }
        Ok(&self.output)
    }
}
