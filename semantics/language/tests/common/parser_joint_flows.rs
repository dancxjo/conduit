use super::parser_kernel;
use conduit_core::StructuredInfoValue;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
pub struct Flow {
    execution: parser_kernel::Execution,
    pub sequence: u64,
    pub calls: u64,
    pub nanos: u128,
    pub maximum_nanos: u128,
}
impl Flow {
    pub fn call(&mut self, input: &StructuredInfoValue) -> StructuredInfoValue {
        let started = std::time::Instant::now();
        let result = self.execution.transact(self.sequence, input);
        let nanos = started.elapsed().as_nanos();
        self.calls += 1;
        self.nanos += nanos;
        self.maximum_nanos = self.maximum_nanos.max(nanos);
        self.sequence += 1;
        result
    }
}
pub struct Pipelines {
    pub flows: Vec<Flow>,
}
impl Pipelines {
    pub fn new(blueprints: &[parser_kernel::Blueprint], epoch: usize) -> Self {
        Self {
            flows: blueprints
                .iter()
                .map(|blueprint| {
                    let mut execution = blueprint.realize(epoch);
                    execution.kernel.start().unwrap();
                    Flow {
                        execution,
                        sequence: 0,
                        calls: 0,
                        nanos: 0,
                        maximum_nanos: 0,
                    }
                })
                .collect(),
        }
    }
    pub fn call(&mut self, stage: usize, input: &StructuredInfoValue) -> StructuredInfoValue {
        self.flows[stage].call(input)
    }
    pub fn merge(
        &mut self,
        beam: LanguageParserJointRuntimeRawBeam,
        proposal: LanguageParserJointRuntimeHypothesis,
    ) -> LanguageParserJointRuntimeRawBeam {
        let input = LanguageParserJointRuntimeMerge::new(beam, proposal).unwrap();
        LanguageParserJointRuntimeRawBeam::from_structured(
            self.call(7, &input.into_structured().unwrap()),
        )
        .unwrap()
    }
}
