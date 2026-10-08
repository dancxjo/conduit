//! Prepared checked Source evaluator and original-frame filter transport.
use alloc::{format, string::String, vec::Vec};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
pub struct PreparedPureExpressionHost {
    evaluator: conduit_plot::PreparedPortableExpressionEvaluator,
    filter_output: Option<PreparedFilterOutput>,
}

enum PreparedFilterOutput {
    Flow(Vec<u8>),
    Value(conduit_core::PreparedOptionalInfoEncoder),
}

impl PreparedPureExpressionHost {
    pub fn prepare(
        program: &conduit_plot::PortableExpressionProgram,
        temporal: conduit_core::PortTemporal,
        filter: bool,
    ) -> Result<Self, String> {
        if filter {
            crate::pure_filter_contract(program, temporal)
        } else {
            crate::pure_expression_contract(program, temporal)
        }
        .map_err(|error| format!("pure Source projection refusal: {error:?}"))?;
        let filter_output = if filter {
            Some(match temporal {
                conduit_core::PortTemporal::Value => PreparedFilterOutput::Value(
                    conduit_core::PreparedOptionalInfoEncoder::new(program.input_type.clone())
                        .map_err(|error| format!("prepare optional filter output: {error:?}"))?,
                ),
                conduit_core::PortTemporal::Flow { .. } => PreparedFilterOutput::Flow(
                    Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
                ),
                conduit_core::PortTemporal::Current => {
                    return Err("when filter does not admit current-value temporal input".into())
                }
            })
        } else {
            None
        };
        Ok(Self {
            evaluator: conduit_plot::PreparedPortableExpressionEvaluator::new(program)
                .map_err(|error| format!("prepare pure expression evaluator: {error:?}"))?,
            filter_output,
        })
    }

    pub fn execute(
        &mut self,
        input: &[u8],
    ) -> Result<&[u8], conduit_plot::PortableExpressionEvaluationRefusal> {
        self.evaluator.evaluate(input)
    }

    pub fn execute_filter(
        &mut self,
        input: &[u8],
    ) -> Result<Option<&[u8]>, conduit_plot::PortableExpressionEvaluationRefusal> {
        let predicate = self.evaluator.evaluate(input)?;
        let selected = conduit_core::InfoBool::decode(predicate)
            .map_err(|_| conduit_plot::PortableExpressionEvaluationRefusal::InvalidProgram)?
            .get();
        match self
            .filter_output
            .as_mut()
            .ok_or(conduit_plot::PortableExpressionEvaluationRefusal::InvalidProgram)?
        {
            PreparedFilterOutput::Flow(output) => {
                if !selected {
                    return Ok(None);
                }
                output.clear();
                output.extend_from_slice(input);
                Ok(Some(output))
            }
            PreparedFilterOutput::Value(output) => output
                .encode(selected.then_some(input))
                .map(Some)
                .map_err(|_| conduit_plot::PortableExpressionEvaluationRefusal::InvalidInput),
        }
    }
}

pub fn program_from_placement(
    placement: &PlannedGear,
) -> Result<conduit_plot::PortableExpressionProgram, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("pure expression requires one exact planned configuration".into());
    };
    let ("program", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value) else {
        return Err("pure expression planned configuration is malformed".into());
    };
    conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|error| format!("pure expression program refusal: {error:?}"))
}
