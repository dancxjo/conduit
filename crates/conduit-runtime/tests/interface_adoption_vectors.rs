use conduit_panel::{LoadedModule, ModuleLoader, resolve_modules};
use conduit_runtime::{Registry, SourceContractCatalog, lower_source_v4};
use std::collections::BTreeMap;

struct MemoryLoader(BTreeMap<String, String>);

impl ModuleLoader for MemoryLoader {
    fn load(&self, canonical_uri: &str) -> Result<Option<LoadedModule>, String> {
        Ok(self.0.get(canonical_uri).map(|source| LoadedModule {
            canonical_uri: canonical_uri.to_owned(),
            source: source.clone(),
        }))
    }
}

#[test]
fn builtin_registry_interfaces_are_present_and_valid() {
    let registry = Registry::compatibility_demo();
    assert!(registry.interface_contract("conduit/stream-sink").is_some());
    assert!(
        registry
            .interface_contract("conduit/text-processor")
            .is_some()
    );

    let sink = registry.interface_contract("conduit/stream-sink").unwrap();
    assert_eq!(sink.id, "conduit/stream-sink");
    assert_eq!(sink.members.len(), 1);
    assert_eq!(sink.members[0].id, "in");

    let proc = registry
        .interface_contract("conduit/text-processor")
        .unwrap();
    assert_eq!(proc.id, "conduit/text-processor");
    assert_eq!(proc.members.len(), 2);
}

#[test]
fn cookbook_interface_primitive_satisfaction_example_lowers_successfully() {
    let source = include_str!("../../../examples/interface-primitive-satisfaction.panel");
    let registry = Registry::compatibility_demo();
    let graph = resolve_modules(
        "mem://example/primitive.panel",
        None,
        &MemoryLoader(BTreeMap::from([(
            "mem://example/primitive.panel".to_owned(),
            source.to_owned(),
        )])),
    )
    .unwrap();

    let lowered = lower_source_v4(&graph, &registry).unwrap();
    assert_eq!(lowered.interface_proofs.len(), 1);
    assert_eq!(
        lowered.interface_proofs[0].interface_id,
        "conduit/stream-sink"
    );
    assert_eq!(lowered.interface_proofs[0].outcome, "compatible");
}

#[test]
fn cookbook_interface_composite_satisfaction_example_lowers_successfully() {
    let source = include_str!("../../../examples/interface-composite-satisfaction.panel");
    let registry = Registry::compatibility_demo();
    let graph = resolve_modules(
        "mem://example/composite.panel",
        None,
        &MemoryLoader(BTreeMap::from([(
            "mem://example/composite.panel".to_owned(),
            source.to_owned(),
        )])),
    )
    .unwrap();

    let lowered = lower_source_v4(&graph, &registry).unwrap();
    assert_eq!(lowered.interface_proofs.len(), 1);
    assert_eq!(
        lowered.interface_proofs[0].interface_id,
        "conduit/text-processor"
    );
    assert_eq!(lowered.interface_proofs[0].outcome, "compatible");
}

#[test]
fn cookbook_interface_consumer_example_lowers_successfully() {
    let source = include_str!("../../../examples/interface-consumer.panel");
    let registry = Registry::compatibility_demo();
    let graph = resolve_modules(
        "mem://example/consumer.panel",
        None,
        &MemoryLoader(BTreeMap::from([(
            "mem://example/consumer.panel".to_owned(),
            source.to_owned(),
        )])),
    )
    .unwrap();

    let lowered = lower_source_v4(&graph, &registry).unwrap();
    assert_eq!(lowered.interface_proofs.len(), 1);
    assert_eq!(
        lowered.interface_proofs[0].interface_id,
        "conduit/stream-sink"
    );
    assert_eq!(lowered.interface_proofs[0].outcome, "compatible");
}

#[test]
fn cookbook_interface_adapter_bridge_example_lowers_successfully() {
    let source = include_str!("../../../examples/interface-adapter-bridge.panel");
    let registry = Registry::compatibility_demo();
    let graph = resolve_modules(
        "mem://example/adapter.panel",
        None,
        &MemoryLoader(BTreeMap::from([(
            "mem://example/adapter.panel".to_owned(),
            source.to_owned(),
        )])),
    )
    .unwrap();

    let lowered = lower_source_v4(&graph, &registry).unwrap();
    assert_eq!(lowered.interface_proofs.len(), 1);
    assert_eq!(
        lowered.interface_proofs[0].interface_id,
        "conduit/text-processor"
    );
    assert_eq!(lowered.interface_proofs[0].outcome, "compatible");
}

#[test]
fn cookbook_interface_diagnostic_failure_example_triggers_rejection() {
    let source = include_str!("../../../examples/interface-diagnostic-failure.panel");
    let registry = Registry::compatibility_demo();
    let graph = resolve_modules(
        "mem://example/failure.panel",
        None,
        &MemoryLoader(BTreeMap::from([(
            "mem://example/failure.panel".to_owned(),
            source.to_owned(),
        )])),
    )
    .unwrap();

    let err = lower_source_v4(&graph, &registry).unwrap_err();
    assert_eq!(err.code, "CND-LWR-013");
}
