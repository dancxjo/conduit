//! Existing kernel fixture supplies actual typed Source/Plan/Play observations.
#[path = "parser_kernel.rs"]
mod parser_kernel;
#[path = "parser_selection_io.rs"]
mod parser_selection_io;
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_language::analysis_epoch_owner::fallback_round::{
    FallbackObservationProvider, FallbackRoundRefusal,
};
use conduitos::protocol_source::*;
use std::collections::BTreeMap;
struct Live {
    execution: parser_kernel::Execution,
    sequence: u64,
}
pub struct Runtime {
    source: PreparedProtocolSource,
    artifact: ArtifactId,
    live: BTreeMap<String, Live>,
    pub operations: u64,
    pub source_id: SourceDocumentId,
}
impl Runtime {
    pub fn new() -> Self {
        let source = [
            parser_selection_io::language_source(),
            include_str!("../../dependency_commitment.conduit").into(),
            include_str!("../../prosody_commitment.conduit").into(),
            include_str!("../../analysis_epoch_initialization.conduit").into(),
            include_str!("../../analysis_fallback_round.conduit").into(),
        ]
        .join("\n");
        let source_id = conduit_plot::parse_syntax_document(&source).source_document_id();
        let package = ProtocolSourcePackage::compile(source, &[]).unwrap();
        let digest = semantic_digest(
            "language/epoch-fallback-runtime-fixture@1",
            &serde_json::to_vec(&package).unwrap(),
        );
        let artifact = ArtifactId::from(format!(
            "language/epoch-fallback/{}",
            digest
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ));
        Self {
            source: PreparedProtocolSource::prepare(package).unwrap(),
            artifact,
            live: BTreeMap::new(),
            operations: 0,
            source_id,
        }
    }
    pub fn execute(&mut self, operation: &str, input: &[u8]) -> Vec<u8> {
        if !self.live.contains_key(operation) {
            let mut execution =
                parser_kernel::Execution::prepare_source(&self.source, operation, &self.artifact);
            assert!(verify_plan(&execution.kernel.definition().internal_plan));
            assert!(!execution.kernel.start().unwrap().is_empty());
            self.live.insert(
                operation.into(),
                Live {
                    execution,
                    sequence: 0,
                },
            );
        }
        let live = self.live.get_mut(operation).unwrap();
        let boundary = &live.execution.kernel.definition().boundary;
        let input_port = boundary.input_fronts[0].external_port.clone();
        let output_port = boundary.output_fronts[0].external_port.clone();
        let value = ValuePayload {
            value_kind: input_port.value_kind,
            encoded: input.to_vec(),
        };
        assert!(matches!(
            live.execution
                .kernel
                .admit_input(&input_port.port_id, live.sequence, &value)
                .unwrap(),
            RemoteIngressOutcome::Accepted { .. }
        ));
        let mut output = ValuePayload {
            value_kind: output_port.value_kind,
            encoded: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        };
        for _ in 0..parser_selection_io::MAXIMUM_SOURCE_STEPS {
            live.execution.step();
            if let Some(sequence) = live
                .execution
                .kernel
                .output_into(&output_port.port_id, &mut output)
                .unwrap()
            {
                assert_eq!(sequence, live.sequence);
                live.execution
                    .kernel
                    .complete_output(&output_port.port_id, sequence)
                    .unwrap();
                live.sequence += 1;
                self.operations += 1;
                return output.encoded;
            }
        }
        panic!("bounded actual Source operation failed: {operation}")
    }
    pub fn assert_signs(&self) {
        assert!(!self.live.is_empty());
        assert!(self
            .live
            .values()
            .all(|run| !run.execution.kernel.active_plays().is_empty()));
        assert!(self.live.values().all(|run| run
            .execution
            .kernel
            .signs()
            .values()
            .flatten()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted)));
    }
}
impl FallbackObservationProvider for Runtime {
    fn observe(&mut self, operation: &str, input: &[u8]) -> Result<Vec<u8>, FallbackRoundRefusal> {
        Ok(self.execute(operation, input))
    }
}
