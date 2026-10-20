//! Test-only ordinary checked Source graph admission and acknowledged output.
#![allow(dead_code)]
use super::parser_kernel::Execution;
use conduit_core::{StructuredInfoValue, ValuePayload, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_plot::rust_binding::NativeRustBinding;

#[path = "parser_selection_trace.rs"]
mod trace;
pub const MAXIMUM_SOURCE_STEPS: usize = 4000;

/// Complete current language Source, including the optional scorer contracts.
/// The ordering matches the semantic binding build entrance.
pub fn language_source() -> String {
    [
        include_str!("../../types.conduit"),
        include_str!("../../identity.conduit"),
        include_str!("../../coverage.conduit"),
        include_str!("../../syntax.conduit"),
        include_str!("../../text_revision.conduit"),
        include_str!("../../revision_lineage.conduit"),
        include_str!("../../lexical.conduit"),
        include_str!("../../pronunciation_selection.conduit"),
        include_str!("../../lexical_consumer.conduit"),
        include_str!("../../parser.conduit"),
        include_str!("../../parser_beam.conduit"),
        include_str!("../../parser_available.conduit"),
        include_str!("../../parser_revision_rebase.conduit"),
        include_str!("../../parser_beam_commit.conduit"),
        include_str!("../../parser_scorer_features.conduit"),
        include_str!("../../parser_scorer_search.conduit"),
        include_str!("../../parser_scorer_actions.conduit"),
        include_str!("../../parser_scorer_ranking.conduit"),
        include_str!("../../parser_scorer_tournament.conduit"),
        include_str!("../../discourse.conduit"),
        include_str!("../../prosody.conduit"),
    ]
    .join("\n")
}

/// A single started Plan/Play and strictly increasing ingress sequence.
/// Inputs and outputs use their actual checked boundary types and canonical
/// frames. This helper supplies transport only, with no linguistic policy.
pub struct SourceGraph {
    entry: String,
    execution: Execution,
    sequence: u64,
    closed: bool,
    output: ValuePayload,
}
impl SourceGraph {
    pub fn prepare(source: String, entry: &str) -> Self {
        Self::from_execution(Execution::prepare(source, entry), entry)
    }
    pub fn from_package(
        package: &conduitos::protocol_source::ProtocolSourcePackage,
        entry: &str,
    ) -> Self {
        Self::from_execution(Execution::prepare_package(package, entry), entry)
    }
    pub fn from_source(
        source: &conduitos::protocol_source::PreparedProtocolSource,
        entry: &str,
        artifact: &conduit_core::ArtifactId,
    ) -> Self {
        Self::from_execution(Execution::prepare_source(source, entry, artifact), entry)
    }
    fn from_execution(mut execution: Execution, entry: &str) -> Self {
        let boundary = &execution.kernel.definition().boundary;
        assert_eq!(boundary.input_fronts.len(), 1, "one graph input");
        assert_eq!(boundary.output_fronts.len(), 1, "one graph output");
        let output = ValuePayload {
            value_kind: boundary.output_fronts[0].external_port.value_kind.clone(),
            encoded: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        };
        execution.kernel.start().unwrap();
        Self {
            entry: entry.into(),
            execution,
            sequence: 0,
            closed: false,
            output,
        }
    }
    pub fn language(entry: &str) -> Self {
        Self::prepare(language_source(), entry)
    }
    pub fn run<T: NativeRustBinding>(&mut self, input: T) -> StructuredInfoValue {
        StructuredInfoValue::from_canonical_bytes(
            &self.run_encoded(input.encode().expect("native input admission")),
        )
        .expect("structured graph output")
    }
    pub fn run_value(&mut self, input: &StructuredInfoValue) -> StructuredInfoValue {
        StructuredInfoValue::from_canonical_bytes(
            &self.run_encoded(input.canonical_bytes().expect("structured input encoding")),
        )
        .expect("structured graph output")
    }
    pub fn run_native<T: NativeRustBinding, U: NativeRustBinding>(&mut self, input: T) -> U {
        U::from_structured(self.run(input)).expect("native output admission")
    }
    pub fn run_bool<T: NativeRustBinding>(&mut self, input: T) -> bool {
        match self
            .run_encoded(input.encode().expect("native input admission"))
            .as_slice()
        {
            [0] => false,
            [1] => true,
            other => panic!("noncanonical Boolean output: {other:?}"),
        }
    }
    fn run_encoded(&mut self, encoded: Vec<u8>) -> Vec<u8> {
        assert!(!self.closed, "source graph already closed");
        let boundary = &self.execution.kernel.definition().boundary;
        let input_port = boundary.input_fronts[0].external_port.clone();
        let output_port = boundary.output_fronts[0].external_port.clone();
        let input = ValuePayload {
            value_kind: input_port.value_kind,
            encoded,
        };
        let admission = self
            .execution
            .kernel
            .admit_input(&input_port.port_id, self.sequence, &input)
            .unwrap();
        assert!(
            matches!(admission, RemoteIngressOutcome::Accepted { .. }),
            "sequence {}: {admission:?}",
            self.sequence
        );
        for _ in 0..MAXIMUM_SOURCE_STEPS {
            self.execution.step();
            if let Some(received) = self
                .execution
                .kernel
                .output_into(&output_port.port_id, &mut self.output)
                .unwrap()
            {
                assert_eq!(received, self.sequence, "exact graph output sequence");
                let result = self.output.encoded.clone();
                self.execution
                    .kernel
                    .complete_output(&output_port.port_id, received)
                    .unwrap();
                trace::record(&self.entry, self.sequence, &input.encoded, &result);
                self.sequence = self.sequence.checked_add(1).expect("finite graph sequence");
                return result;
            }
        }
        panic!(
            "Source graph did not settle within {MAXIMUM_SOURCE_STEPS} steps at sequence {}",
            self.sequence
        );
    }
    pub fn invocations(&self) -> u64 {
        self.sequence
    }
    pub fn close(&mut self) {
        assert!(!self.closed, "source graph already closed");
        let port = self.execution.kernel.definition().boundary.input_fronts[0]
            .external_port
            .port_id
            .clone();
        self.execution.kernel.close_input(&port).unwrap();
        self.closed = true;
    }
}
