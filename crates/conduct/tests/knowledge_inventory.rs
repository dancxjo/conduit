use std::path::PathBuf;
use std::process::Command;

use conduit_compile::{InstalledProfile, compile_source};
use conduit_knowledge::{
    register_deterministic_graph_provider, register_deterministic_retrieval_provider,
    register_knowledge_contracts,
};
use conduit_runtime::{AvailabilityState, Registry};

fn workspace_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn exact_retrieval_binding_and_citation_run_through_the_production_executor() {
    let source = include_str!("../../../examples/knowledge-retrieve-cite.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_retrieval_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let retrieval = document
        .nodes
        .iter()
        .find(|node| node.contract.id == "knowledge/retrieve")
        .unwrap();
    assert_eq!(
        retrieval.implementation.id,
        "conduit.knowledge/retrieve-reference"
    );
    assert_eq!(
        retrieval.artifact,
        "conduit.knowledge/retrieve-reference-artifact"
    );

    for (path, expected) in [
        (
            "examples/knowledge-retrieve-cite.panel",
            "knowledge:citation:31..42:exact plans",
        ),
        (
            "examples/knowledge-retrieve-compose.panel",
            "KNOWLEDGE:CITATION:31..42:EXACT PLANS",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg(workspace_file(path))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn contract_only_and_exact_provider_states_remain_distinct() {
    let mut registry = Registry::default();
    register_knowledge_contracts(&mut registry);
    assert_eq!(
        registry.node_availability("knowledge/retrieve").state,
        AvailabilityState::ContractOnly
    );
    assert!(
        registry
            .contracts()
            .all(|contract| !contract.id.as_str().starts_with("ai/")
                && !contract.id.as_str().contains("generate"))
    );
}

#[test]
fn exact_cited_claim_graph_runs_through_the_production_executor() {
    let source = include_str!("../../../examples/knowledge-graph-traverse.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_retrieval_provider(&mut registry).unwrap();
    register_deterministic_graph_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let traversal = document
        .nodes
        .iter()
        .find(|node| node.contract.id == "knowledge/graph/traverse")
        .unwrap();
    assert_eq!(
        traversal.implementation.id,
        "conduit.knowledge/graph-traverse-reference"
    );
    assert_eq!(
        traversal.artifact,
        "conduit.knowledge/graph-traverse-reference-artifact"
    );

    for (path, expected) in [
        (
            "examples/knowledge-graph-traverse.panel",
            "knowledge:graph:Conduit--keeps-distinct-->exact-plans[source:31..42]",
        ),
        (
            "examples/knowledge-graph-compose.panel",
            "KNOWLEDGE:GRAPH:CONDUIT--KEEPS-DISTINCT-->EXACT-PLANS[SOURCE:31..42]",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg(workspace_file(path))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn retrieval_provider_can_be_ready_while_graph_provider_is_absent() {
    let mut registry = Registry::default();
    register_knowledge_contracts(&mut registry);
    register_deterministic_retrieval_provider(&mut registry).unwrap();
    assert_eq!(
        registry.node_availability("knowledge/retrieve").state,
        AvailabilityState::ProviderAvailable
    );
    assert_eq!(
        registry.node_availability("knowledge/graph/traverse").state,
        AvailabilityState::ContractOnly
    );
}
