//! Actual Source execution through the public typed boundary; no parser policy claim.
use conduit_core::{StructuredInfoType, StructuredInfoValue};
use conduit_language::{parser_session_runtime::*, LanguageTextRevisionId};
use conduitos::protocol_source::{PreparedProtocolEntry, ProtocolSourcePackage};
use std::sync::Arc;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;

struct Executor {
    retained: Arc<PreparedProtocolEntry>,
    execution: parser_kernel::Execution,
    input: StructuredInfoType,
    output: StructuredInfoType,
}
impl ParserSourceExecutor for Executor {
    type Error = core::convert::Infallible;
    fn entry(&self) -> &str {
        "language-source-port-custody"
    }
    fn input_type(&self) -> &StructuredInfoType {
        &self.input
    }
    fn output_type(&self) -> &StructuredInfoType {
        &self.output
    }
    fn transact(
        &mut self,
        ordinal: u64,
        input: &StructuredInfoValue,
    ) -> Result<StructuredInfoValue, Self::Error> {
        // Keep the whole checked Source and expansion alive alongside its Plan.
        assert!(!self.retained.expanded().expanded.gears.is_empty());
        Ok(self.execution.transact(ordinal, input))
    }
}
#[test]
fn checked_source_ports_and_retained_plan_execute_two_exact_native_revisions() {
    let source = format!("{}\nplot language-source-port-custody (\n input: LanguageTextRevisionId...| >> output: LanguageTextRevisionId...|\n) = .\n", include_str!("../identity.conduit"));
    let package = ProtocolSourcePackage::compile(source.clone(), &[]).unwrap();
    let retained = Arc::new(
        PreparedProtocolEntry::prepare(
            &serde_json::to_vec(&package).unwrap(),
            "language-source-port-custody",
        )
        .unwrap(),
    );
    let input = retained
        .input_schema(&retained.expanded().front.inputs()[0].port_id)
        .unwrap();
    let output = retained
        .output_schema(&retained.expanded().front.outputs()[0].port_id)
        .unwrap();
    let mut execution = parser_kernel::Execution::prepare(source, "language-source-port-custody");
    execution.kernel.start().unwrap();
    let executor = Executor {
        retained: retained.clone(),
        execution,
        input,
        output,
    };
    let mut flow =
        PreparedParserSourceFlow::<LanguageTextRevisionId, LanguageTextRevisionId, _>::new(
            executor,
        )
        .unwrap();
    drop(retained);
    for identity in ["source/first", "source/second"] {
        let revision = LanguageTextRevisionId::new(identity.into()).unwrap();
        assert_eq!(flow.transact(revision.clone()).unwrap(), revision);
    }
    assert_eq!(flow.next_ordinal(), 2);
    assert!(!flow.is_poisoned());
}
