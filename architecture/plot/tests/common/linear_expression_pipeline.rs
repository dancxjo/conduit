//! Prepared evaluator fixture for an exact ordinary linear Source composition.
use conduit_core::{ConfigurationValue, StructuredInfoType};
use conduit_plot::*;
pub struct Programs {
    pub input_type: StructuredInfoType,
    stages: Vec<PortableExpressionProgram>,
}
impl Programs {
    pub fn new(source: &str, entry: &str) -> Self {
        let checked =
            check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
        let mut gear = expanded.input_bindings[0].gear_id.clone();
        let mut stages = Vec::new();
        loop {
            let node = expanded
                .expanded
                .gears
                .iter()
                .find(|node| node.gear_id == gear)
                .unwrap();
            let ConfigurationValue::Text(encoded) = &node.configuration[0].value else {
                panic!("expression program")
            };
            stages.push(PortableExpressionProgram::from_canonical_hex(encoded).unwrap());
            if expanded
                .output_bindings
                .iter()
                .any(|binding| binding.gear_id == gear)
            {
                break;
            }
            let next: Vec<_> = expanded
                .expanded
                .connections
                .iter()
                .filter(|cord| cord.source_gear_id == gear)
                .collect();
            assert_eq!(
                next.len(),
                1,
                "fixture accepts only an exact linear pipeline"
            );
            gear = next[0].sink_gear_id.clone();
            assert!(stages.len() < expanded.expanded.gears.len());
        }
        assert_eq!(stages.len(), expanded.expanded.gears.len());
        Self {
            input_type: stages[0].input_type.clone(),
            stages,
        }
    }
    pub fn prepare(&self) -> Prepared {
        Prepared {
            stages: self
                .stages
                .iter()
                .map(|stage| PreparedPortableExpressionEvaluator::new(stage).unwrap())
                .collect(),
            first: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            second: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }
    pub fn evaluate(&self, input: &[u8]) -> Result<Vec<u8>, PortableExpressionEvaluationRefusal> {
        Ok(self.prepare().evaluate(input)?.to_vec())
    }
}
pub struct Prepared {
    stages: Vec<PreparedPortableExpressionEvaluator>,
    first: Vec<u8>,
    second: Vec<u8>,
}
impl Prepared {
    pub fn evaluate(&mut self, input: &[u8]) -> Result<&[u8], PortableExpressionEvaluationRefusal> {
        self.first.clear();
        self.first.extend_from_slice(input);
        for stage in &mut self.stages {
            let output = stage.evaluate(&self.first)?;
            self.second.clear();
            self.second.extend_from_slice(output);
            core::mem::swap(&mut self.first, &mut self.second);
        }
        Ok(&self.first)
    }
}
